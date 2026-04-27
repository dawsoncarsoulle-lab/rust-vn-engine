// rvn_bevy/src/systems/error_overlay.rs
//
// Overlay d'erreur affiché quand le moteur rencontre une erreur runtime.
// Non-dismissable — force l'auteur à voir et corriger le problème.
//
// Format affiché :
//
//   ╔══ ERREUR DE SCRIPT ══════════════════════════════════════════╗
//   ║ erreur d'exécution au statement #42                          ║
//   ║  |                                                           ║
//   ║  | SetVar { name: "score", value: Var("points") }            ║
//   ║  |                                                           ║
//   ║  = variable non définie : `points`                           ║
//   ║    aide : utilise `set points = <valeur>` avant              ║
//   ║                                                              ║
//   ║  Corrige le script et relance le jeu.                        ║
//   ╚══════════════════════════════════════════════════════════════╝

use crate::resources::ScriptErrorMessage;
use bevy::prelude::*;

#[derive(Component)]
pub struct ErrorOverlay;

pub fn spawn_error_overlay(mut commands: Commands, error_msg: Res<ScriptErrorMessage>) {
    let message = if error_msg.0.is_empty() {
        "Erreur inconnue.".to_string()
    } else {
        error_msg.0.clone()
    };

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
                    padding: UiRect::all(Val::Px(40.0)),
                    ..default()
                },
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.92).into(),
                z_index: ZIndex::Global(9999),
                ..default()
            },
            ErrorOverlay,
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(32.0)),
                        max_width: Val::Px(860.0),
                        border: UiRect::all(Val::Px(2.0)),
                        row_gap: Val::Px(16.0),
                        ..default()
                    },
                    background_color: Color::srgba(0.12, 0.03, 0.03, 1.0).into(),
                    border_color: Color::srgba(0.85, 0.15, 0.15, 1.0).into(),
                    ..default()
                })
                .with_children(|card| {
                    card.spawn(TextBundle::from_section(
                        "⚠  ERREUR DE SCRIPT",
                        TextStyle {
                            font_size: 22.0,
                            color: Color::srgba(1.0, 0.35, 0.35, 1.0),
                            ..default()
                        },
                    ));

                    card.spawn(TextBundle::from_section(
                        message,
                        TextStyle {
                            font_size: 16.0,
                            color: Color::srgba(0.95, 0.85, 0.85, 1.0),
                            ..default()
                        },
                    ));

                    card.spawn(TextBundle::from_section(
                        "Corrige le script et relance le jeu.",
                        TextStyle {
                            font_size: 14.0,
                            color: Color::srgba(0.6, 0.6, 0.6, 1.0),
                            ..default()
                        },
                    ));
                });
        });
}

pub fn despawn_error_overlay(mut commands: Commands, query: Query<Entity, With<ErrorOverlay>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
