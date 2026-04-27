use bevy::prelude::*;
use rvn_core::save::SaveManager;
use rvn_parser::Transition;

use crate::resources::{
    DialogueHistory, ImagemapState, Theme, TypewriterState, VnEngine, VnRenderState, VnState,
};
use crate::vn_command::VnCommand;

// ─── Composants locaux ───────────────────────────────────────────────────────

#[derive(Component)]
pub struct TitleOverlay;

#[derive(Component, Clone, PartialEq)]
pub enum TitleButton {
    NewGame,
    LoadGame,
    Quit,
}

// ─── Spawn / Despawn ─────────────────────────────────────────────────────────

pub fn spawn_title_screen(
    mut commands: Commands,
    theme: Res<Theme>,
    asset_server: Res<AssetServer>,
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
            spawn_title_button(parent, "Quitter", TitleButton::Quit, title_font);
        });
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
                    TitleButton::NewGame => {
                        info!("[titre] nouvelle partie");
                        next_state.set(VnState::Stepping);
                    }

                    TitleButton::LoadGame => {
                        let save_dir = std::env::current_dir().unwrap_or_default().join("saves");
                        if let Ok(mgr) = SaveManager::new(&save_dir, 5) {
                            if engine.0.load(&mgr, 1).is_ok() {
                                info!("[titre] partie chargée depuis slot 1");
                                render_state.choice_options.clear();
                                imagemap_state.clear();
                                tw_state.skip();
                                history.clear();

                                let mut pending = engine.0.renderer.take_pending();
                                for cmd in pending.iter_mut() {
                                    match cmd {
                                        VnCommand::SetBackground { transition, .. } => {
                                            *transition = Transition::None
                                        }
                                        VnCommand::ShowSprite { transition, .. } => {
                                            *transition = Transition::None
                                        }
                                        VnCommand::HideSprite { transition, .. } => {
                                            *transition = Transition::None
                                        }
                                        VnCommand::MoveSprite { transition, .. } => {
                                            *transition = Transition::None
                                        }
                                        _ => {}
                                    }
                                }
                                for cmd in pending {
                                    vn_events.send(cmd);
                                }

                                next_state.set(VnState::Waiting);
                            } else {
                                error!("[titre] aucune sauvegarde en slot 1");
                            }
                        }
                    }

                    TitleButton::Quit => {
                        exit.send(AppExit::Success);
                    }
                }
            }
        }
    }
}
