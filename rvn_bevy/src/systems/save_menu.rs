//! Systems and components to display a save/load menu overlay.
//!
//! The save menu appears on top of the main pause menu when the player
//! chooses to save or load.  It lists several save slots along with
//! metadata such as timestamp and label.  Selecting a slot will save
//! or load depending on the current mode.  The menu can be cancelled
//! to return to the pause menu without taking any action.

use std::time::{Duration, UNIX_EPOCH};

use bevy::prelude::*;
use chrono::{DateTime, Local};
use rvn_core::save::{SaveData, SaveManager};

use crate::project_paths::ProjectPaths;
use crate::resources::{
    DialogueHistory, ImagemapState, MenuState, TypewriterState, VnEngine, VnRenderState, VnState,
};
use crate::vn_command::VnCommand;
use rvn_parser::Transition;

/// Mode of the save menu: Save or Load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveMenuMode {
    Save,
    Load,
}

/// Resource tracking whether the save menu is active and which mode it is in.
#[derive(Resource, Debug)]
pub struct SaveMenuState {
    pub active: bool,
    pub mode: SaveMenuMode,
}

impl Default for SaveMenuState {
    fn default() -> Self {
        Self {
            active: false,
            mode: SaveMenuMode::Save,
        }
    }
}

/// Component marking the root entity of the save menu overlay.
#[derive(Component)]
pub struct SaveMenuOverlay;

/// Component on each save slot button.  Holds the slot index (starting at 1).
#[derive(Component)]
pub struct SaveSlotButton(pub usize);

/// Component on the cancel button.
#[derive(Component)]
pub struct SaveMenuCancelButton;

/// Maximum number of save slots to display.
pub const MAX_SLOTS: usize = 5;

pub fn apply_loaded_game(
    engine: &mut VnEngine,
    render_state: &mut VnRenderState,
    imagemap_state: &mut ImagemapState,
    tw_state: &mut TypewriterState,
    history: &mut DialogueHistory,
    vn_events: &mut EventWriter<VnCommand>,
) {
    render_state.choice_options.clear();
    imagemap_state.clear();
    tw_state.skip();
    history.clear();

    let mut pending = engine.0.renderer.take_pending();
    for cmd in pending.iter_mut() {
        match cmd {
            VnCommand::SetBackground { transition, .. } => *transition = Transition::None,
            VnCommand::ShowSprite { transition, .. } => *transition = Transition::None,
            VnCommand::HideSprite { transition, .. } => *transition = Transition::None,
            VnCommand::MoveSprite { transition, .. } => *transition = Transition::None,
            _ => {}
        }
    }
    for cmd in pending {
        vn_events.send(cmd);
    }
}

/// Spawn the save menu overlay when becoming active.
pub fn spawn_save_menu_overlay(
    mut commands: Commands,
    save_state: Res<SaveMenuState>,
    project_paths: Res<ProjectPaths>,
    _engine: Res<VnEngine>,
    query: Query<Entity, With<SaveMenuOverlay>>,
) {
    // Only spawn when becoming active and overlay is not already present
    if !save_state.active {
        return;
    }
    if !query.is_empty() {
        return;
    }

    // Build a SaveManager to list existing saves
    let save_mgr = match SaveManager::new(&project_paths.saves, MAX_SLOTS as u32) {
        Ok(mgr) => mgr,
        Err(e) => {
            error!("[save_menu] failed to create SaveManager: {e}");
            return;
        }
    };
    // Gather metadata for each slot
    let mut slots: Vec<Option<SaveData>> = Vec::new();
    for slot_index in 1..=MAX_SLOTS {
        match save_mgr.load(slot_index as u32) {
            Ok(data) => slots.push(Some(data)),
            Err(_) => slots.push(None),
        }
    }

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
            SaveMenuOverlay,
        ))
        .with_children(|parent| {
            // Title
            let title_text = match save_state.mode {
                SaveMenuMode::Save => "Sauvegarder",
                SaveMenuMode::Load => "Charger",
            };
            parent.spawn(TextBundle::from_section(
                title_text,
                TextStyle {
                    font_size: 40.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            // Slots
            for (i, meta_opt) in slots.iter().enumerate() {
                let slot_index = i + 1;
                let label_text: String;
                let timestamp_text: String;
                if let Some(meta) = meta_opt {
                    label_text = meta.label.clone();
                    let ts = UNIX_EPOCH + Duration::from_secs(meta.timestamp as u64);
                    let datetime: DateTime<Local> = ts.into();
                    timestamp_text = datetime.format("%d/%m/%Y %H:%M").to_string();
                } else {
                    label_text = format!("Slot {} (vide)", slot_index);
                    timestamp_text = "".to_string();
                }

                parent
                    .spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Px(400.0),
                                height: Val::Px(60.0),
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(8.0)),
                                ..default()
                            },
                            background_color: Color::srgba(0.10, 0.10, 0.25, 0.95).into(),
                            ..default()
                        },
                        SaveSlotButton(slot_index),
                    ))
                    .with_children(|btn| {
                        // Label text
                        btn.spawn(TextBundle::from_section(
                            label_text.clone(),
                            TextStyle {
                                font_size: 24.0,
                                color: Color::WHITE,
                                ..default()
                            },
                        ));
                        // Timestamp text
                        if !timestamp_text.is_empty() {
                            btn.spawn(TextBundle::from_section(
                                timestamp_text.clone(),
                                TextStyle {
                                    font_size: 16.0,
                                    color: Color::srgba(0.5, 0.5, 0.5, 1.0),
                                    ..default()
                                },
                            ));
                        }
                    });
            }

            // Cancel button
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
                    SaveMenuCancelButton,
                ))
                .with_children(|btn| {
                    btn.spawn(TextBundle::from_section(
                        "Annuler",
                        TextStyle {
                            font_size: 22.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    ));
                });
        });
}

/// Handle interactions with the save menu overlay.
pub fn save_menu_interaction_system(
    mut interaction_query: Query<
        (
            &Interaction,
            Option<&SaveSlotButton>,
            Option<&SaveMenuCancelButton>,
            &mut BackgroundColor,
        ),
        Changed<Interaction>,
    >,
    mut save_state: ResMut<SaveMenuState>,
    mut next_state: ResMut<NextState<VnState>>,
    mut menu_state: ResMut<MenuState>,
    project_paths: Res<ProjectPaths>,
    mut engine: ResMut<VnEngine>,
    mut render_state: ResMut<VnRenderState>,
    mut imagemap_state: ResMut<ImagemapState>,
    mut tw_state: ResMut<TypewriterState>,
    mut history: ResMut<DialogueHistory>,
    mut vn_events: EventWriter<VnCommand>,
) {
    if !save_state.active {
        return;
    }

    for (interaction, slot_comp, cancel_comp, mut bg_color) in interaction_query.iter_mut() {
        match interaction {
            Interaction::Hovered => {
                *bg_color = Color::srgba(0.20, 0.20, 0.40, 0.95).into();
            }
            Interaction::None => {
                *bg_color = Color::srgba(0.10, 0.10, 0.25, 0.95).into();
            }
            Interaction::Pressed => {
                *bg_color = Color::srgba(0.30, 0.30, 0.50, 0.95).into();
                if let Some(_) = cancel_comp {
                    // Cancel: close menu
                    save_state.active = false;
                    if menu_state.return_to == Some(VnState::TitleScreen) {
                        menu_state.return_to = None;
                        next_state.set(VnState::TitleScreen);
                    }
                    return;
                }
                if let Some(slot) = slot_comp {
                    let slot_index = slot.0;
                    // Create SaveManager
                    match SaveManager::new(&project_paths.saves, MAX_SLOTS as u32) {
                        Ok(mgr) => match save_state.mode {
                            SaveMenuMode::Save => {
                                let label = format!("Sauvegarde {}", slot_index);
                                let script_name = "script.rvn".to_string();
                                match engine.0.save(&mgr, slot_index as u32, label, script_name) {
                                    Ok(_) => info!("[save_menu] saved slot {}", slot_index),
                                    Err(e) => error!(
                                        "[save_menu] error saving slot {}: {}",
                                        slot_index, e
                                    ),
                                }
                                save_state.active = false;
                            }
                            SaveMenuMode::Load => {
                                match engine.0.load(&mgr, slot_index as u32) {
                                    Ok(_) => {
                                        info!("[save_menu] loaded slot {}", slot_index);
                                        apply_loaded_game(
                                            &mut engine,
                                            &mut render_state,
                                            &mut imagemap_state,
                                            &mut tw_state,
                                            &mut history,
                                            &mut vn_events,
                                        );
                                        menu_state.return_to = None;
                                        next_state.set(VnState::Waiting);
                                    }
                                    Err(e) => error!(
                                        "[save_menu] error loading slot {}: {}",
                                        slot_index, e
                                    ),
                                }
                                save_state.active = false;
                            }
                        },
                        Err(e) => error!("[save_menu] failed to create SaveManager: {}", e),
                    }
                }
            }
        }
    }
}

/// Despawn the save menu overlay when it is no longer active.
pub fn despawn_save_menu_overlay(
    mut commands: Commands,
    save_state: Res<SaveMenuState>,
    overlay_query: Query<Entity, With<SaveMenuOverlay>>,
) {
    if save_state.active {
        return;
    }
    for entity in overlay_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
