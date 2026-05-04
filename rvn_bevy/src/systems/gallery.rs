use bevy::prelude::*;

use crate::resources::{CgAssetRegistry, GalleryState, PersistentDataResource, VnState};

#[derive(Component)]
pub struct GalleryOverlay;

#[derive(Component, Clone)]
pub enum GalleryButton {
    Back,
    Cg(String),
}

pub fn spawn_gallery_overlay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    persistent: Res<PersistentDataResource>,
    cg_registry: Res<CgAssetRegistry>,
    state: Res<GalleryState>,
    query: Query<Entity, With<GalleryOverlay>>,
) {
    if !query.is_empty() {
        return;
    }

    let selected_path = state
        .selected_cg
        .as_ref()
        .and_then(|id| cg_registry.0.get(id).cloned());

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
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(14.0),
                    padding: UiRect::all(Val::Px(24.0)),
                    ..default()
                },
                background_color: Color::srgba(0.03, 0.03, 0.06, 1.0).into(),
                ..default()
            },
            GalleryOverlay,
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "Galerie",
                TextStyle {
                    font_size: 48.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            if let Some(path) = selected_path {
                parent.spawn(ImageBundle {
                    style: Style {
                        width: Val::Percent(86.0),
                        height: Val::Percent(72.0),
                        ..default()
                    },
                    image: UiImage::new(asset_server.load(path)),
                    ..default()
                });
            } else if persistent.data.seen_cgs.is_empty() {
                parent.spawn(TextBundle::from_section(
                    "Aucune CG débloquée",
                    TextStyle {
                        font_size: 24.0,
                        color: Color::srgb(0.75, 0.75, 0.8),
                        ..default()
                    },
                ));
            } else {
                parent
                    .spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(80.0),
                            max_width: Val::Px(900.0),
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(12.0),
                            column_gap: Val::Px(12.0),
                            ..default()
                        },
                        background_color: Color::NONE.into(),
                        ..default()
                    })
                    .with_children(|grid| {
                        for id in &persistent.data.seen_cgs {
                            if cg_registry.0.contains_key(id) {
                                spawn_gallery_button(grid, id, GalleryButton::Cg(id.clone()));
                            }
                        }
                    });
            }

            spawn_gallery_button(parent, "Retour", GalleryButton::Back);
        });
}

fn spawn_gallery_button(parent: &mut ChildBuilder, label: &str, action: GalleryButton) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(260.0),
                    height: Val::Px(54.0),
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
                    font_size: 24.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
}

pub fn gallery_interaction_system(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &GalleryButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    overlay_query: Query<Entity, With<GalleryOverlay>>,
    mut state: ResMut<GalleryState>,
    mut next_state: ResMut<NextState<VnState>>,
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
                    GalleryButton::Back if state.selected_cg.is_some() => {
                        state.selected_cg = None;
                        for entity in overlay_query.iter() {
                            commands.entity(entity).despawn_recursive();
                        }
                    }
                    GalleryButton::Back => {
                        next_state.set(VnState::TitleScreen);
                    }
                    GalleryButton::Cg(id) => {
                        state.selected_cg = Some(id.clone());
                        for entity in overlay_query.iter() {
                            commands.entity(entity).despawn_recursive();
                        }
                    }
                }
            }
        }
    }
}

pub fn despawn_gallery_overlay(
    mut commands: Commands,
    mut state: ResMut<GalleryState>,
    query: Query<Entity, With<GalleryOverlay>>,
) {
    state.selected_cg = None;
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
