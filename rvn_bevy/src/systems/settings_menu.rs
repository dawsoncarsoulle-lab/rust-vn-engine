//! Systems and components to display and interact with the settings menu overlay.
//!
//! The settings menu allows players to adjust audio volumes, text speed, auto‑advance
//! speed, toggle the typewriter effect, switch language, and toggle fullscreen.
//! The menu is shown from the pause menu and overlays the game.  When the
//! settings menu is open, game input is suspended until it is closed.

use bevy::prelude::*;

use crate::components::DialogueText;
use crate::resources::{
    MenuState, MusicVolume, PersistentDataResource, TypewriterConfig, TypewriterState, VnEngine,
    VnState,
};
use crate::systems::typewriter::apply_visible_sections;

/// Configuration for various runtime settings.  Values are normalised between
/// sensible ranges (0.0‑1.0 for volumes, etc.).  The language string should
/// correspond to the keys in your `locales/` directory (e.g. "fr" or "en").
#[derive(Resource, Clone)]
pub struct Settings {
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub text_speed: f32,
    pub typewriter: bool,
    pub language: String,
    pub fullscreen: bool,
    pub auto_speed: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            music_volume: 1.0,
            sfx_volume: 1.0,
            text_speed: 1.0,
            typewriter: true,
            language: "fr".to_string(),
            fullscreen: false,
            auto_speed: 1.0,
        }
    }
}

/// Tracks whether the settings menu overlay is currently visible.
#[derive(Resource, Default, Debug)]
pub struct SettingsMenuState {
    pub active: bool,
}

/// Marker component for the root entity of the settings menu overlay.
#[derive(Component)]
pub struct SettingsMenuOverlay;

/// Buttons in the settings menu.  Each variant corresponds to an action such as
/// increasing volume or toggling a boolean flag.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsButton {
    MusicUp,
    MusicDown,
    SfxUp,
    SfxDown,
    TextUp,
    TextDown,
    AutoUp,
    AutoDown,
    TypewriterToggle,
    LangNext,
    FullscreenToggle,
    Close,
}

/// Text fields whose displayed value mirrors the `Settings` resource.
/// These components let us refresh the value labels in place without
/// despawning/recreating the whole Settings overlay, avoiding visual jumps.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsValueText {
    MusicVolume,
    SfxVolume,
    TextSpeed,
    AutoSpeed,
    Typewriter,
    Language,
    Fullscreen,
}

/// Spawns the settings menu overlay when the `SettingsMenuState` becomes active.
pub fn spawn_settings_menu_overlay(
    mut commands: Commands,
    settings_state: Res<SettingsMenuState>,
    settings: Res<Settings>,
    query: Query<Entity, With<SettingsMenuOverlay>>,
    _windows: Query<&Window>,
) {
    if !settings_state.active {
        return;
    }
    // Only spawn if not already present
    if !query.is_empty() {
        return;
    }

    // Build overlay root
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.90).into(),
                z_index: ZIndex::Global(2000),
                ..default()
            },
            SettingsMenuOverlay,
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "Paramètres",
                TextStyle {
                    font_size: 40.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            // Music volume row
            spawn_setting_row(
                parent,
                "Volume musique",
                settings.music_volume,
                SettingsButton::MusicDown,
                SettingsButton::MusicUp,
                SettingsValueText::MusicVolume,
            );
            // SFX volume row
            spawn_setting_row(
                parent,
                "Volume effets",
                settings.sfx_volume,
                SettingsButton::SfxDown,
                SettingsButton::SfxUp,
                SettingsValueText::SfxVolume,
            );
            // Text speed row
            spawn_setting_row(
                parent,
                "Vitesse texte",
                settings.text_speed,
                SettingsButton::TextDown,
                SettingsButton::TextUp,
                SettingsValueText::TextSpeed,
            );
            // Auto speed row
            spawn_setting_row(
                parent,
                "Vitesse auto",
                settings.auto_speed,
                SettingsButton::AutoDown,
                SettingsButton::AutoUp,
                SettingsValueText::AutoSpeed,
            );
            // Typewriter toggle row
            spawn_toggle_row(
                parent,
                "Typewriter",
                if settings.typewriter {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
                Some(settings.typewriter),
                SettingsButton::TypewriterToggle,
                SettingsValueText::Typewriter,
            );
            // Language row
            spawn_toggle_row(
                parent,
                "Langue",
                settings.language.clone(),
                None,
                SettingsButton::LangNext,
                SettingsValueText::Language,
            );
            // Fullscreen toggle row
            spawn_toggle_row(
                parent,
                "Plein écran",
                if settings.fullscreen {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
                Some(settings.fullscreen),
                SettingsButton::FullscreenToggle,
                SettingsValueText::Fullscreen,
            );

            // Close button
            parent
                .spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Px(200.0),
                            height: Val::Px(40.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::all(Val::Px(12.0)),
                            ..default()
                        },
                        background_color: Color::srgba(0.15, 0.15, 0.30, 0.95).into(),
                        ..default()
                    },
                    SettingsButton::Close,
                ))
                .with_children(|btn| {
                    btn.spawn(TextBundle::from_section(
                        "Fermer",
                        TextStyle {
                            font_size: 20.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    ));
                });
        });
}

/// Helper to spawn a row with a label, value and plus/minus buttons.
fn spawn_setting_row(
    parent: &mut ChildBuilder,
    label: &str,
    value: f32,
    dec_button: SettingsButton,
    inc_button: SettingsButton,
    value_kind: SettingsValueText,
) {
    parent
        .spawn(NodeBundle {
            style: Style {
                width: Val::Px(500.0),
                height: Val::Px(40.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            background_color: Color::srgba(0.10, 0.10, 0.25, 0.95).into(),
            ..default()
        })
        .with_children(|row| {
            // Label
            row.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
            // Value display. This text is tagged so it can be updated in place.
            row.spawn((
                TextBundle::from_section(
                    format!("{:.1}", value),
                    TextStyle {
                        font_size: 20.0,
                        color: Color::srgba(0.5, 0.5, 0.5, 1.0),
                        ..default()
                    },
                ),
                value_kind,
            ));
            // Buttons
            row.spawn((
                ButtonBundle {
                    style: Style {
                        width: Val::Px(30.0),
                        height: Val::Px(30.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    background_color: Color::srgba(0.20, 0.20, 0.40, 0.95).into(),
                    ..default()
                },
                dec_button,
            ))
            .with_children(|btn| {
                btn.spawn(TextBundle::from_section(
                    "-",
                    TextStyle {
                        font_size: 16.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ));
            });
            row.spawn((
                ButtonBundle {
                    style: Style {
                        width: Val::Px(30.0),
                        height: Val::Px(30.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    background_color: Color::srgba(0.20, 0.20, 0.40, 0.95).into(),
                    ..default()
                },
                inc_button,
            ))
            .with_children(|btn| {
                btn.spawn(TextBundle::from_section(
                    "+",
                    TextStyle {
                        font_size: 16.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ));
            });
        });
}

/// Helper to spawn a row with a label and a toggle button.  The current state is
/// represented by a boolean; clicking the button toggles the value.
fn spawn_toggle_row(
    parent: &mut ChildBuilder,
    label: &str,
    value_label: String,
    state_for_color: Option<bool>,
    toggle_button: SettingsButton,
    value_kind: SettingsValueText,
) {
    parent
        .spawn(NodeBundle {
            style: Style {
                width: Val::Px(500.0),
                height: Val::Px(40.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            background_color: Color::srgba(0.10, 0.10, 0.25, 0.95).into(),
            ..default()
        })
        .with_children(|row| {
            row.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
            let value_color = match state_for_color {
                Some(true) => Color::srgba(0.2, 0.9, 0.2, 1.0),
                Some(false) => Color::srgba(0.9, 0.2, 0.2, 1.0),
                None => Color::srgba(0.5, 0.5, 0.9, 1.0),
            };
            row.spawn((
                TextBundle::from_section(
                    value_label,
                    TextStyle {
                        font_size: 20.0,
                        color: value_color,
                        ..default()
                    },
                ),
                value_kind,
            ));
            row.spawn((
                ButtonBundle {
                    style: Style {
                        width: Val::Px(60.0),
                        height: Val::Px(30.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    background_color: Color::srgba(0.20, 0.20, 0.40, 0.95).into(),
                    ..default()
                },
                toggle_button,
            ))
            .with_children(|btn| {
                btn.spawn(TextBundle::from_section(
                    "Toggle",
                    TextStyle {
                        font_size: 14.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ));
            });
        });
}

/// Handles button interactions within the settings menu.  Adjusts settings
/// values and toggles flags based on which button was clicked.  Closes the
/// overlay when the Close button is pressed.
pub fn settings_menu_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &SettingsButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut settings_state: ResMut<SettingsMenuState>,
    mut settings: ResMut<Settings>,
    mut persistent: ResMut<PersistentDataResource>,
    mut windows: Query<&mut Window>,
) {
    for (interaction, button) in interaction_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            match button {
                SettingsButton::MusicUp => {
                    settings.music_volume = (settings.music_volume + 0.1).min(1.0);
                }
                SettingsButton::MusicDown => {
                    settings.music_volume = (settings.music_volume - 0.1).max(0.0);
                }
                SettingsButton::SfxUp => {
                    settings.sfx_volume = (settings.sfx_volume + 0.1).min(1.0);
                }
                SettingsButton::SfxDown => {
                    settings.sfx_volume = (settings.sfx_volume - 0.1).max(0.0);
                }
                SettingsButton::TextUp => {
                    settings.text_speed = (settings.text_speed + 0.1).min(5.0);
                }
                SettingsButton::TextDown => {
                    settings.text_speed = (settings.text_speed - 0.1).max(0.1);
                }
                SettingsButton::AutoUp => {
                    settings.auto_speed = (settings.auto_speed + 0.1).min(5.0);
                }
                SettingsButton::AutoDown => {
                    settings.auto_speed = (settings.auto_speed - 0.1).max(0.1);
                }
                SettingsButton::TypewriterToggle => {
                    settings.typewriter = !settings.typewriter;
                }
                SettingsButton::LangNext => {
                    // Very simple language toggle: fr <-> en
                    if settings.language == "fr" {
                        settings.language = "en".to_string();
                    } else {
                        settings.language = "fr".to_string();
                    }
                }
                SettingsButton::FullscreenToggle => {
                    settings.fullscreen = !settings.fullscreen;
                    // Apply fullscreen change immediately
                    if let Ok(mut win) = windows.get_single_mut() {
                        win.mode = if settings.fullscreen {
                            bevy::window::WindowMode::BorderlessFullscreen
                        } else {
                            bevy::window::WindowMode::Windowed
                        };
                    }
                }
                SettingsButton::Close => {
                    settings_state.active = false;
                }
            }

            if !matches!(button, SettingsButton::Close) {
                persistent.data.language = Some(settings.language.clone());
                persistent.data.music_volume = Some(settings.music_volume);
                persistent.data.sfx_volume = Some(settings.sfx_volume);
                persistent.data.text_speed = Some(settings.text_speed);
                persistent.data.auto_speed = Some(settings.auto_speed);
                persistent.data.fullscreen = Some(settings.fullscreen);
                if let Err(e) = persistent.manager.save(&persistent.data) {
                    error!("[settings] impossible de sauvegarder persistent.json : {e}");
                }
            }
        }
    }
}

/// Refreshes Settings value labels in place when the Settings resource changes.
/// This avoids despawning and respawning the overlay on every button click.
pub fn update_settings_value_text_system(
    settings: Res<Settings>,
    mut query: Query<(&SettingsValueText, &mut Text)>,
) {
    if !settings.is_changed() {
        return;
    }

    for (kind, mut text) in query.iter_mut() {
        let value = match kind {
            SettingsValueText::MusicVolume => format!("{:.1}", settings.music_volume),
            SettingsValueText::SfxVolume => format!("{:.1}", settings.sfx_volume),
            SettingsValueText::TextSpeed => format!("{:.1}", settings.text_speed),
            SettingsValueText::AutoSpeed => format!("{:.1}", settings.auto_speed),
            SettingsValueText::Typewriter => {
                if settings.typewriter {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            }
            SettingsValueText::Language => settings.language.clone(),
            SettingsValueText::Fullscreen => {
                if settings.fullscreen {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            }
        };

        text.sections[0].value = value;
        text.sections[0].style.color = match kind {
            SettingsValueText::Typewriter | SettingsValueText::Fullscreen => {
                let enabled = match kind {
                    SettingsValueText::Typewriter => settings.typewriter,
                    SettingsValueText::Fullscreen => settings.fullscreen,
                    _ => false,
                };
                if enabled {
                    Color::srgba(0.2, 0.9, 0.2, 1.0)
                } else {
                    Color::srgba(0.9, 0.2, 0.2, 1.0)
                }
            }
            SettingsValueText::Language => Color::srgba(0.5, 0.5, 0.9, 1.0),
            _ => Color::srgba(0.5, 0.5, 0.5, 1.0),
        };
    }
}

/// Applies user-facing settings to the actual runtime resources.
///
/// The Settings menu only owns the UI-facing configuration. This system bridges
/// it to the systems that actually render text and play audio:
/// - `music_volume` updates `MusicVolume`, which `audio_system` applies to the active music sink.
/// - `sfx_volume` is read directly by `audio_system` whenever a new SFX is spawned.
/// - `typewriter` and `text_speed` update `TypewriterConfig`.
///
/// When typewriter is disabled while a line is currently animating, the current
/// line is immediately completed so the player sees the effect instantly.
pub fn apply_settings_to_runtime_system(
    settings: Res<Settings>,
    mut music_volume: ResMut<MusicVolume>,
    mut tw_config: ResMut<TypewriterConfig>,
    mut tw_state: ResMut<TypewriterState>,
    mut engine: ResMut<VnEngine>,
    mut windows: Query<&mut Window>,
    mut dialogue_text_query: Query<&mut Text, With<DialogueText>>,
) {
    if !settings.is_changed() {
        return;
    }

    // Audio: music is driven through MusicVolume; SFX uses Settings directly when spawned.
    music_volume.0 = settings.music_volume.clamp(0.0, 1.0);

    if let Some(locale) = &mut engine.0.locale {
        if locale.current_lang() != settings.language {
            if let Err(e) = locale.set_language(&settings.language) {
                error!(
                    "[settings] impossible de charger la langue `{}` : {}",
                    settings.language, e
                );
            }
        }
    }

    if let Ok(mut win) = windows.get_single_mut() {
        win.mode = if settings.fullscreen {
            bevy::window::WindowMode::BorderlessFullscreen
        } else {
            bevy::window::WindowMode::Windowed
        };
    }

    // Text: convert the UI multiplier into a chars-per-second value.
    let cps = if settings.typewriter {
        (40.0 * settings.text_speed).max(1.0)
    } else {
        0.0
    };
    tw_config.enabled = settings.typewriter;
    tw_config.chars_per_sec = cps;

    // If a line is already typing, apply the setting immediately.
    if !settings.typewriter {
        tw_state.skip();
        if let Ok(mut text) = dialogue_text_query.get_single_mut() {
            apply_visible_sections(&mut text, &tw_state);
        }
    } else if tw_state.typing {
        tw_state.chars_per_sec = cps;
    }
}

/// Despawn the settings menu overlay if the menu is not active and the overlay exists.
pub fn despawn_settings_menu_overlay(
    mut commands: Commands,
    settings_state: Res<SettingsMenuState>,
    current_state: Res<State<VnState>>,
    mut next_state: ResMut<NextState<VnState>>,
    mut menu_state: ResMut<MenuState>,
    query: Query<Entity, With<SettingsMenuOverlay>>,
) {
    if settings_state.active {
        return;
    }
    if let Ok(ent) = query.get_single() {
        commands.entity(ent).despawn_recursive();
        if current_state.get() == &VnState::Menu
            && menu_state.return_to == Some(VnState::TitleScreen)
        {
            menu_state.return_to = None;
            next_state.set(VnState::TitleScreen);
        }
    }
}
