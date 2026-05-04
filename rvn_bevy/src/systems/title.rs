use bevy::prelude::*;
use rvn_core::save::SaveManager;

use crate::project_paths::ProjectPaths;
use crate::resources::{
    DialogueHistory, ImagemapState, MenuState, Theme, TypewriterState, VnEngine, VnRenderState,
    VnState,
};
use crate::systems::save_menu::{apply_loaded_game, SaveMenuMode, SaveMenuState, MAX_SLOTS};
use crate::vn_command::VnCommand;

// ─── Composants locaux ───────────────────────────────────────────────────────

#[derive(Component)]
pub struct TitleOverlay;

#[derive(Component, Clone, PartialEq)]
pub enum TitleButton {
    Continue,
    NewGame,
    LoadGame,
    Gallery,
    Quit,
}

// ─── Spawn / Despawn ─────────────────────────────────────────────────────────

pub fn spawn_title_screen(
    mut commands: Commands,
    theme: Res<Theme>,
    asset_server: Res<AssetServer>,
    project_paths: Res<ProjectPaths>,
) {
    let title_font: Handle<Font> = theme
        .text
        .name
        .font_path
        .as_ref()
        .map(|p| asset_server.load(p.clone()))
        .unwrap_or_default();

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
                    row_gap: Val::Px(20.0),
                    ..default()
                },
                background_color: Color::srgba(0.05, 0.05, 0.1, 1.0).into(),
                ..default()
            },
            TitleOverlay,
        ))
        .with_children(|parent| {
            parent.spawn(
                TextBundle::from_section(
                    "MON VISUAL NOVEL",
                    TextStyle {
                        font: title_font.clone(),
                        font_size: 80.0,
                        color: Color::WHITE,
                    },
                )
                .with_style(Style {
                    margin: UiRect::bottom(Val::Px(40.0)),
                    ..default()
                }),
            );

            if title_continue_available(&project_paths) {
                spawn_title_button(
                    parent,
                    "Continuer",
                    TitleButton::Continue,
                    title_font.clone(),
                );
            }
            spawn_title_button(
                parent,
                "Nouvelle Partie",
                TitleButton::NewGame,
                title_font.clone(),
            );
            spawn_title_button(
                parent,
                "Charger la Partie",
                TitleButton::LoadGame,
                title_font.clone(),
            );
            spawn_title_button(parent, "Galerie", TitleButton::Gallery, title_font.clone());
            spawn_title_button(parent, "Quitter", TitleButton::Quit, title_font);
        });
}

fn title_continue_available(project_paths: &ProjectPaths) -> bool {
    SaveManager::new(&project_paths.saves, MAX_SLOTS as u32)
        .map(|mgr| mgr.autosave_exists() || mgr.latest_manual_save().is_some())
        .unwrap_or(false)
}

fn spawn_title_button(
    parent: &mut ChildBuilder,
    label: &str,
    action: TitleButton,
    font: Handle<Font>,
) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(350.0),
                    height: Val::Px(60.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: Color::srgba(0.15, 0.15, 0.25, 1.0).into(),
                ..default()
            },
            action,
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font,
                    font_size: 30.0,
                    color: Color::WHITE,
                },
            ));
        });
}

pub fn despawn_title_screen(
    mut commands: Commands,
    overlay_query: Query<Entity, With<TitleOverlay>>,
) {
    for entity in overlay_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

// ─── Interaction ─────────────────────────────────────────────────────────────

pub fn title_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &TitleButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut next_state: ResMut<NextState<VnState>>,
    mut engine: ResMut<VnEngine>,
    mut render_state: ResMut<VnRenderState>,
    mut imagemap_state: ResMut<ImagemapState>,
    mut tw_state: ResMut<TypewriterState>,
    mut history: ResMut<DialogueHistory>,
    project_paths: Res<ProjectPaths>,
    mut menu_state: ResMut<MenuState>,
    mut save_menu_state: ResMut<SaveMenuState>,
    mut vn_events: EventWriter<VnCommand>,
    mut exit: EventWriter<AppExit>,
) {
    for (interaction, button, mut bg_color) in interaction_query.iter_mut() {
        match interaction {
            Interaction::Hovered => {
                *bg_color = Color::srgba(0.25, 0.25, 0.45, 1.0).into();
            }
            Interaction::None => {
                *bg_color = Color::srgba(0.15, 0.15, 0.25, 1.0).into();
            }
            Interaction::Pressed => {
                *bg_color = Color::srgba(0.35, 0.35, 0.65, 1.0).into();

                match button {
                    TitleButton::Continue => {
                        match SaveManager::new(&project_paths.saves, MAX_SLOTS as u32) {
                            Ok(mgr) => {
                                let data = if mgr.autosave_exists() {
                                    mgr.load_autosave()
                                } else {
                                    mgr.latest_manual_save()
                                        .ok_or(rvn_core::save::SaveError::SlotVide(0))
                                };

                                match data {
                                    Ok(data) => {
                                        engine.0.load_data(data);
                                        apply_loaded_game(
                                            &mut engine,
                                            &mut render_state,
                                            &mut imagemap_state,
                                            &mut tw_state,
                                            &mut history,
                                            &mut vn_events,
                                        );
                                        next_state.set(VnState::Waiting);
                                    }
                                    Err(e) => error!("[titre] reprise impossible: {e}"),
                                }
                            }
                            Err(e) => error!("[titre] SaveManager indisponible: {e}"),
                        }
                    }

                    TitleButton::NewGame => {
                        info!("[titre] nouvelle partie");
                        next_state.set(VnState::Stepping);
                    }

                    TitleButton::LoadGame => {
                        menu_state.return_to = Some(VnState::TitleScreen);
                        save_menu_state.mode = SaveMenuMode::Load;
                        save_menu_state.active = true;
                        next_state.set(VnState::Menu);
                    }

                    TitleButton::Gallery => {
                        next_state.set(VnState::Gallery);
                    }

                    TitleButton::Quit => {
                        exit.send(AppExit::Success);
                    }
                }
            }
        }
    }
}
