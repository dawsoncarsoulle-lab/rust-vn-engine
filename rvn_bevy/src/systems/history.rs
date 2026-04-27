use bevy::prelude::*;

use crate::resources::{DialogueHistory, Theme, VnState};

// ─── Composant local ─────────────────────────────────────────────────────────

#[derive(Component)]
pub struct HistoryOverlay;

// ─── Spawn / Despawn ─────────────────────────────────────────────────────────

pub fn spawn_history_overlay(
    mut commands: Commands,
    history: Res<DialogueHistory>,
    theme: Res<Theme>,
    asset_server: Res<AssetServer>,
) {
    let name_font: Handle<Font> = theme
        .text
        .name
        .font_path
        .as_ref()
        .map(|p| asset_server.load(p.clone()))
        .unwrap_or_default();
    let text_font: Handle<Font> = theme
        .text
        .dialogue
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
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(40.0)),
                    justify_content: JustifyContent::FlexEnd,
                    ..default()
                },
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.85).into(),
                z_index: ZIndex::Global(900),
                ..default()
            },
            HistoryOverlay,
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(20.0),
                        overflow: Overflow::clip_y(),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|scroll_parent| {
                    for line in &history.lines {
                        scroll_parent
                            .spawn(NodeBundle {
                                style: Style {
                                    flex_direction: FlexDirection::Column,
                                    width: Val::Percent(100.0),
                                    ..default()
                                },
                                ..default()
                            })
                            .with_children(|item| {
                                if !line.character.is_empty() {
                                    item.spawn(
                                        TextBundle::from_section(
                                            &line.character,
                                            TextStyle {
                                                font: name_font.clone(),
                                                font_size: 20.0,
                                                color: Color::srgba(0.8, 0.8, 1.0, 1.0),
                                            },
                                        )
                                        .with_style(
                                            Style {
                                                margin: UiRect::bottom(Val::Px(5.0)),
                                                ..default()
                                            },
                                        ),
                                    );
                                }
                                item.spawn(TextBundle::from_section(
                                    &line.text,
                                    TextStyle {
                                        font: text_font.clone(),
                                        font_size: 24.0,
                                        color: Color::WHITE,
                                    },
                                ));
                            });
                    }
                });
        });
}

pub fn despawn_history_overlay(
    mut commands: Commands,
    overlay_query: Query<Entity, With<HistoryOverlay>>,
) {
    for entity in overlay_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

// ─── Input ───────────────────────────────────────────────────────────────────

pub fn history_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut next_state: ResMut<NextState<VnState>>,
) {
    if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Right) {
        next_state.set(VnState::Waiting);
    }
}
