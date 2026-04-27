// rvn_bevy/src/systems/debug_overlay.rs
//
// Système d'overlay de debug pour le moteur RVN.
//
// Ce module fournit un overlay affichant des informations de débogage en jeu,
// notamment le compteur de programme (PC), le statement courant et les
// variables du moteur. L'affichage peut être activé/désactivé à l'aide de la
// touche F9.  L'overlay est mis à jour chaque frame lorsque l'option est
// active.

use bevy::prelude::*;

use crate::resources::VnEngine;

/// Ressource permettant de savoir si l'overlay de debug est visible.
#[derive(Resource, Default)]
pub struct DebugOverlayState {
    pub visible: bool,
}

/// Composant marqueur pour le noeud racine de l'overlay.
#[derive(Component)]
pub struct DebugOverlay;

/// Composant marqueur pour le texte mis à jour dynamiquement.
#[derive(Component)]
pub struct DebugOverlayText;

/// Système qui écoute la touche F9 et bascule l'overlay de debug.
pub fn debug_toggle_system(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<DebugOverlayState>) {
    if keys.just_pressed(KeyCode::F9) {
        state.visible = !state.visible;
    }
}

/// Ressource utilisée pour demander une exécution d'une étape lorsqu'on est en mode debug.
#[derive(Resource, Default)]
pub struct DebugStepRequest {
    pub pending: bool,
}

/// Système qui met à jour le DebugStepRequest lorsque la touche F10 est pressée.
/// Ce système n'active la demande de step que si l'overlay de debug est visible.
pub fn debug_step_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    overlay_state: Res<DebugOverlayState>,
    mut req: ResMut<DebugStepRequest>,
) {
    if overlay_state.visible && keys.just_pressed(KeyCode::F10) {
        req.pending = true;
    }
}

/// Système qui spawne ou despawne l'overlay en fonction de `DebugOverlayState`.
pub fn spawn_or_despawn_debug_overlay_system(
    mut commands: Commands,
    state: Res<DebugOverlayState>,
    query: Query<Entity, With<DebugOverlay>>,
) {
    if state.visible {
        // Afficher l'overlay s'il n'existe pas déjà.
        if query.is_empty() {
            // Construit un fond semi‑transparent couvrant tout l'écran.
            commands
                .spawn((
                    NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::FlexStart,
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        background_color: Color::srgba(0.0, 0.0, 0.0, 0.5).into(),
                        z_index: ZIndex::Global(9998),
                        ..default()
                    },
                    DebugOverlay,
                ))
                .with_children(|parent| {
                    // Carte contenant le texte de debug.
                    parent
                        .spawn(NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::all(Val::Px(12.0)),
                                max_width: Val::Px(400.0),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            background_color: Color::srgba(0.1, 0.1, 0.1, 0.8).into(),
                            border_color: Color::srgba(0.6, 0.6, 0.6, 1.0).into(),
                            ..default()
                        })
                        .with_children(|card| {
                            // Titre de la carte.
                            card.spawn(TextBundle::from_section(
                                "Debug RVN",
                                TextStyle {
                                    font_size: 18.0,
                                    color: Color::srgba(0.9, 0.9, 0.9, 1.0),
                                    ..default()
                                },
                            ));
                            // Texte dynamique mis à jour par update_debug_overlay_system.
                            card.spawn((
                                TextBundle::from_section(
                                    "Initialisation…",
                                    TextStyle {
                                        font_size: 14.0,
                                        color: Color::srgba(0.8, 0.8, 0.8, 1.0),
                                        ..default()
                                    },
                                ),
                                DebugOverlayText,
                            ));
                        });
                });
        }
    } else {
        // Masquer l'overlay s'il est présent.
        for entity in query.iter() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

/// Système qui met à jour le contenu de l'overlay à chaque frame.
pub fn update_debug_overlay_system(
    engine: Res<VnEngine>,
    state: Res<DebugOverlayState>,
    mut query: Query<&mut Text, With<DebugOverlayText>>,
) {
    if !state.visible {
        return;
    }
    // Récupère des infos du moteur : compteur de programme, statement et variables.
    let pc = engine.0.state.pc;
    let stmt_str = engine
        .0
        .script
        .get(pc)
        .map(|s| format!("{:?}", s))
        .unwrap_or_else(|| "<fin de script>".to_string());
    let mut vars_str = String::new();
    for (k, v) in &engine.0.state.vars {
        vars_str.push_str(&format!("{} = {:?}\n", k, v));
    }
    if vars_str.is_empty() {
        vars_str.push_str("<aucune variable>\n");
    }
    let full = format!(
        "PC : {}\nStmt : {}\nVariables :\n{}",
        pc, stmt_str, vars_str
    );
    for mut text in query.iter_mut() {
        // Remplace le texte entier de la première section.
        if let Some(section) = text.sections.get_mut(0) {
            section.value = full.clone();
        }
    }
}