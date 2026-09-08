use bevy::prelude::*;

use crate::resources::{
    CgAssetRegistry, GalleryState, GalleryView, PersistentDataResource, VnState,
};

#[derive(Component)]
pub struct GalleryOverlay;
#[derive(Component)]pub(crate) struct GalleryColors{pub normal:Color,pub hover:Color,pub pressed:Color}

#[derive(Component, Clone)]
pub enum GalleryButton {
    Back,
    ShowCg,
    ShowEndings,
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

            if selected_path.is_none() {
                parent
                    .spawn(NodeBundle {
                        style: Style {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(12.0),
                            ..default()
                        },
                        background_color: Color::NONE.into(),
                        ..default()
                    })
                    .with_children(|tabs| {
                        spawn_gallery_tab_button(
                            tabs,
                            "CG",
                            GalleryButton::ShowCg,
                            state.view == GalleryView::Cg,
                        );
                        spawn_gallery_tab_button(
                            tabs,
                            "Endings",
                            GalleryButton::ShowEndings,
                            state.view == GalleryView::Endings,
                        );
                    });
            }

            if let Some(path) = selected_path {
                spawn_selected_cg(parent, asset_server.load(path));
            } else {
                match state.view {
                    GalleryView::Cg => spawn_cg_grid(parent, &persistent, &cg_registry),
                    GalleryView::Endings => spawn_ending_list(parent, &persistent),
                }
            }

            spawn_gallery_button(parent, "Retour", GalleryButton::Back);
        });
}

fn spawn_selected_cg(parent: &mut ChildBuilder, image: Handle<Image>) {
    parent.spawn(ImageBundle {
        style: Style {
            width: Val::Percent(86.0),
            height: Val::Percent(72.0),
            ..default()
        },
        image: UiImage::new(image),
        ..default()
    });
}

fn spawn_cg_grid(
    parent: &mut ChildBuilder,
    persistent: &PersistentDataResource,
    cg_registry: &CgAssetRegistry,
) {
    let unlocked_cgs: Vec<&String> = persistent
        .data
        .seen_cgs
        .iter()
        .filter(|id| cg_registry.0.contains_key(*id))
        .collect();

    if unlocked_cgs.is_empty() {
        spawn_gallery_empty_message(parent, "Aucune CG débloquée");
        return;
    }

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
            for id in unlocked_cgs {
                spawn_gallery_button(grid, id, GalleryButton::Cg(id.clone()));
            }
        });
}

fn spawn_ending_list(parent: &mut ChildBuilder, persistent: &PersistentDataResource) {
    if persistent.data.seen_endings.is_empty() {
        spawn_gallery_empty_message(parent, "Aucune ending débloquée");
        return;
    }

    parent
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(74.0),
                max_width: Val::Px(720.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Stretch,
                row_gap: Val::Px(10.0),
                ..default()
            },
            background_color: Color::NONE.into(),
            ..default()
        })
        .with_children(|list| {
            for id in &persistent.data.seen_endings {
                spawn_ending_entry(list, id);
            }
        });
}

fn spawn_ending_entry(parent: &mut ChildBuilder, id: &str) {
    parent
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                min_height: Val::Px(58.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexStart,
                padding: UiRect::axes(Val::Px(18.0), Val::Px(8.0)),
                ..default()
            },
            background_color: Color::srgba(0.12, 0.12, 0.2, 1.0).into(),
            ..default()
        })
        .with_children(|entry| {
            entry.spawn(TextBundle::from_section(
                format_ending_label(id),
                TextStyle {
                    font_size: 24.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
            entry.spawn(TextBundle::from_section(
                id,
                TextStyle {
                    font_size: 16.0,
                    color: Color::srgb(0.65, 0.65, 0.72),
                    ..default()
                },
            ));
        });
}

fn spawn_gallery_empty_message(parent: &mut ChildBuilder, message: &str) {
    parent.spawn(TextBundle::from_section(
        message,
        TextStyle {
            font_size: 24.0,
            color: Color::srgb(0.75, 0.75, 0.8),
            ..default()
        },
    ));
}

fn spawn_gallery_tab_button(
    parent: &mut ChildBuilder,
    label: &str,
    action: GalleryButton,
    active: bool,
) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(150.0),
                    height: Val::Px(44.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: if active {
                    Color::srgba(0.35, 0.35, 0.65, 1.0).into()
                } else {
                    Color::srgba(0.15, 0.15, 0.25, 1.0).into()
                },
                ..default()
            },
            action,
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font_size: 22.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
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
        (&Interaction, &GalleryButton, &mut BackgroundColor,Option<&GalleryColors>),
        (Changed<Interaction>, With<Button>),
    >,
    overlay_query: Query<Entity, With<GalleryOverlay>>,
    mut state: ResMut<GalleryState>,
    mut next_state: ResMut<NextState<VnState>>,
) {
    for (interaction, button, mut bg_color,colors) in interaction_query.iter_mut() {
        match interaction {
            Interaction::Hovered => {
                *bg_color = colors.map(|c|c.hover).unwrap_or(Color::srgba(0.25, 0.25, 0.45, 1.0)).into();
            }
            Interaction::None => {
                *bg_color = colors.map(|c|c.normal).unwrap_or_else(||gallery_button_color(button, &state)).into();
            }
            Interaction::Pressed => {
                *bg_color = colors.map(|c|c.pressed).unwrap_or(Color::srgba(0.35, 0.35, 0.65, 1.0)).into();
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
                    GalleryButton::ShowCg => {
                        state.view = GalleryView::Cg;
                        state.selected_cg = None;
                        for entity in overlay_query.iter() {
                            commands.entity(entity).despawn_recursive();
                        }
                    }
                    GalleryButton::ShowEndings => {
                        state.view = GalleryView::Endings;
                        state.selected_cg = None;
                        for entity in overlay_query.iter() {
                            commands.entity(entity).despawn_recursive();
                        }
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

fn gallery_button_color(button: &GalleryButton, state: &GalleryState) -> Color {
    match button {
        GalleryButton::ShowCg if state.view == GalleryView::Cg => {
            Color::srgba(0.35, 0.35, 0.65, 1.0)
        }
        GalleryButton::ShowEndings if state.view == GalleryView::Endings => {
            Color::srgba(0.35, 0.35, 0.65, 1.0)
        }
        _ => Color::srgba(0.15, 0.15, 0.25, 1.0),
    }
}

pub fn despawn_gallery_overlay(
    mut commands: Commands,
    mut state: ResMut<GalleryState>,
    query: Query<Entity, With<GalleryOverlay>>,
) {
    state.view = GalleryView::Cg;
    state.selected_cg = None;
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn format_ending_label(id: &str) -> String {
    id.split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.flat_map(char::to_lowercase))
                    .collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ending_labels_are_title_cased_from_ids() {
        assert_eq!(format_ending_label("demo_end"), "Demo End");
        assert_eq!(format_ending_label("good_true_end"), "Good True End");
        assert_eq!(format_ending_label("__bad__end__"), "Bad End");
    }
}
