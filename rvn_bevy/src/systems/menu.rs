use bevy::prelude::*;
// SaveManager n'est plus utilisé ici ; le menu n'accède plus aux fichiers directement.
use rvn_parser::Transition;

use crate::resources::{
    DialogueHistory, ImagemapState, MenuState, TypewriterState, VnEngine, VnRenderState, VnState,
};
use crate::systems::save_menu::{SaveMenuState, SaveMenuMode};
use crate::systems::settings_menu::SettingsMenuState;
use crate::vn_command::VnCommand;

// ─── Composants locaux ───────────────────────────────────────────────────────

#[derive(Component)]
pub struct MenuOverlay;

#[derive(Component, Clone, PartialEq)]
pub enum MenuButton {
    Resume,
    Save,
    Load,
    Settings,
    Quit,
}

// ─── Spawn / Despawn ─────────────────────────────────────────────────────────

pub fn spawn_menu_overlay(mut commands: Commands) {
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
                    row_gap: Val::Px(16.0),
                    ..default()
                },
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.80).into(),
                z_index: ZIndex::Global(1000),
                ..default()
            },
            MenuOverlay,
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "PAUSE",
                TextStyle {
                    font_size: 48.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            spawn_menu_button(parent, "Reprendre", MenuButton::Resume);
            spawn_menu_button(parent, "Sauvegarder", MenuButton::Save);
            spawn_menu_button(parent, "Charger", MenuButton::Load);
            spawn_menu_button(parent, "Paramètres", MenuButton::Settings);
            spawn_menu_button(parent, "Quitter", MenuButton::Quit);
        });
}

fn spawn_menu_button(parent: &mut ChildBuilder, label: &str, action: MenuButton) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(320.0),
                    height: Val::Px(56.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: Color::srgba(0.08, 0.08, 0.20, 0.95).into(),
                ..default()
            },
            action,
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font_size: 26.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
}

pub fn despawn_menu_overlay(
    mut commands: Commands,
    overlay_query: Query<Entity, With<MenuOverlay>>,
) {
    for entity in overlay_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

// ─── Interaction ─────────────────────────────────────────────────────────────

pub fn menu_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &MenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut next_state: ResMut<NextState<VnState>>,
    mut menu_state: ResMut<MenuState>,
    mut engine: ResMut<VnEngine>,
    mut render_state: ResMut<VnRenderState>,
    mut imagemap_state: ResMut<ImagemapState>,
    mut tw_state: ResMut<TypewriterState>,
    mut history: ResMut<DialogueHistory>,
    mut vn_events: EventWriter<VnCommand>,
    mut exit: EventWriter<AppExit>,
    mut save_menu_state: ResMut<SaveMenuState>,
    mut settings_menu_state: ResMut<SettingsMenuState>,
) {
    // When a sub-menu is open, do not let clicks pass through to the pause menu
    // behind it. This prevents opening "Charger" while "Paramètres" is already
    // active, and vice versa.
    if save_menu_state.active || settings_menu_state.active {
        return;
    }

    for (interaction, button, mut bg_color) in interaction_query.iter_mut() {
        match interaction {
            Interaction::Hovered => {
                *bg_color = Color::srgba(0.15, 0.15, 0.35, 0.95).into();
            }
            Interaction::None => {
                *bg_color = Color::srgba(0.08, 0.08, 0.20, 0.95).into();
            }
            Interaction::Pressed => {
                *bg_color = Color::srgba(0.25, 0.25, 0.55, 0.95).into();

                match button {
                    MenuButton::Resume => {
                        let ret = menu_state.return_to.take().unwrap_or(VnState::Waiting);
                        next_state.set(ret);
                    }

                    MenuButton::Save => {
                        // Open save menu overlay in Save mode
                        save_menu_state.mode = SaveMenuMode::Save;
                        save_menu_state.active = true;
                    }

                    MenuButton::Load => {
                        // Open save menu overlay in Load mode
                        save_menu_state.mode = SaveMenuMode::Load;
                        save_menu_state.active = true;
                    }

                    MenuButton::Settings => {
                        // Open settings overlay
                        settings_menu_state.active = true;
                    }

                    MenuButton::Quit => {
                        exit.send(AppExit::Success);
                    }
                }
            }
        }
    }
}
