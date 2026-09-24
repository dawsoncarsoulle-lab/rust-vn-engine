//! Native popup for data-bound selectors, with isolated keyboard focus.
use super::*;

#[derive(Component, Clone)]
pub(crate) struct LanguageSelect {
    pub normal: Color,
    pub hover: Color,
    pub selected: Color,
    pub foreground: Color,
    pub text_states: rvn_ui::TextStateColors,
    pub font: Handle<Font>,
    pub font_size: f32,
    pub(super) scrollbar: scrollbars::ScrollAppearance,
}
#[derive(Component)]
pub(super) struct Popup;
#[derive(Component)]
pub(super) struct OptionRow(usize);
#[derive(Component)]
pub(super) struct PopupList;
#[derive(Resource, Default)]
pub(crate) struct Dropdown {
    pub open: bool,
    source: Option<Entity>,
    options: Vec<String>,
    selected: usize,
    bounds: [f32; 4],
    reveal: bool,
}

/// Popup labels use the same state palette as the selector that opened them.
pub(super) fn text_colors(
    state: Res<Dropdown>,
    selectors: Query<&LanguageSelect>,
    rows: Query<(&OptionRow, &Interaction)>,
    mut texts: Query<(&Parent, &mut Text)>,
) {
    if !state.open {
        return;
    }
    let Some(appearance) = state.source.and_then(|id| selectors.get(id).ok()) else {
        return;
    };
    for (parent, mut text) in &mut texts {
        let Ok((row, interaction)) = rows.get(parent.get()) else {
            continue;
        };
        let selected = row.0 == state.selected;
        let value = color(appearance.text_states.resolve(
            appearance.foreground.to_srgba().to_f32_array(),
            true,
            *interaction == Interaction::Pressed,
            *interaction == Interaction::Hovered,
            false,
            selected,
        ));
        if text.sections.iter().any(|s| s.style.color != value) {
            for section in &mut text.sections {
                section.style.color = value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn popup_text_uses_source_palette_without_changing_it() {
        let mut app = App::new();
        app.init_resource::<Dropdown>()
            .add_systems(Update, text_colors);
        let e = Element::new("language".into(), Kind::Select);
        let palette = rvn_ui::TextStateColors {
            hover: Some([1.0, 0.0, 0.0, 1.0]),
            selected: Some([0.0, 1.0, 0.0, 1.0]),
            ..default()
        };
        let source = app
            .world_mut()
            .spawn(LanguageSelect {
                normal: Color::BLACK,
                hover: Color::BLACK,
                selected: Color::BLACK,
                foreground: Color::WHITE,
                text_states: palette.clone(),
                font: default(),
                font_size: 20.0,
                scrollbar: scrollbars::ScrollAppearance::from_element(&e),
            })
            .id();
        let row = app
            .world_mut()
            .spawn((OptionRow(0), Interaction::None))
            .id();
        let text = app
            .world_mut()
            .spawn(TextBundle::from_section("Français", TextStyle::default()))
            .id();
        app.world_mut().entity_mut(row).add_child(text);
        {
            let mut state = app.world_mut().resource_mut::<Dropdown>();
            state.open = true;
            state.source = Some(source);
            state.selected = 0;
        }
        app.update();
        assert_eq!(
            app.world().get::<Text>(text).unwrap().sections[0]
                .style
                .color,
            color([0.0, 1.0, 0.0, 1.0])
        );
        *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Hovered;
        app.update();
        assert_eq!(
            app.world().get::<Text>(text).unwrap().sections[0]
                .style
                .color,
            color([1.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(
            app.world()
                .get::<LanguageSelect>(source)
                .unwrap()
                .text_states,
            palette
        );
    }
}

pub(super) fn reveal_selection(
    mut state: ResMut<Dropdown>,
    mut lists: Query<(&Node, &mut MenuScroll), With<PopupList>>,
    mut rows: Query<(&OptionRow, &mut Style)>,
) {
    if !state.open || !state.reveal {
        return;
    }
    let Ok((node, mut scroll)) = lists.get_single_mut() else {
        return;
    };
    if node.size().y <= 0.0 {
        return;
    }
    let top = state.selected as f32 * 42.0;
    let offset = if top < scroll.offset {
        top
    } else if top + 42.0 > scroll.offset + node.size().y {
        top + 42.0 - node.size().y
    } else {
        scroll.offset
    };
    scroll.offset = offset.clamp(0.0, (scroll.content_height - node.size().y).max(0.0));
    for (row, mut style) in &mut rows {
        style.top = Val::Px(row.0 as f32 * 42.0 - scroll.offset);
    }
    state.reveal = false;
}

pub(super) fn update(
    confirmation: Res<crate::systems::save_menu::SaveConfirmation>,
    mut commands: Commands,
    mut state: ResMut<Dropdown>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    pads: Res<ButtonInput<GamepadButton>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    selectors: Query<(
        Entity,
        &LanguageSelect,
        &Interaction,
        &Node,
        &GlobalTransform,
    )>,
    mut rows: Query<(&OptionRow, &Interaction, &mut BackgroundColor)>,
    roots: Query<Entity, With<Popup>>,
    locales: Res<LocaleConfig>,
    mut settings: ResMut<crate::systems::settings_menu::Settings>,
    mut persistent: ResMut<PersistentDataResource>,
    mut menus: ResMut<Menus>,
    locals: Query<&local_controls::LocalInput>,
) {
    if confirmation.active() {
        return;
    }
    let Ok(window) = windows.get_single() else {
        return;
    };
    let mut close = false;
    let mut choose = false;
    if state.open {
        if state.source.is_none_or(|id| selectors.get(id).is_err()) {
            close = true;
        }
        let pad = |b| pads.get_just_pressed().any(|p| p.button_type == b);
        if keys.just_pressed(KeyCode::Escape) || pad(GamepadButtonType::East) {
            keys.clear_just_pressed(KeyCode::Escape);
            close = true;
        }
        let back = keys.just_pressed(KeyCode::ArrowUp)
            || pad(GamepadButtonType::DPadUp)
            || keys.just_pressed(KeyCode::Tab)
                && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
        let next = keys.just_pressed(KeyCode::ArrowDown)
            || keys.just_pressed(KeyCode::Tab)
            || pad(GamepadButtonType::DPadDown);
        if back || next {
            state.reveal = true;
            let n = state.options.len();
            if n > 0 {
                state.selected = if back {
                    (state.selected + n - 1) % n
                } else {
                    (state.selected + 1) % n
                };
            }
            for key in [KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::Tab] {
                keys.clear_just_pressed(key);
            }
        }
        if keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::Space)
            || pad(GamepadButtonType::South)
        {
            keys.clear_just_pressed(KeyCode::Enter);
            keys.clear_just_pressed(KeyCode::Space);
            choose = true;
        }
        for (row, interaction, _) in &rows {
            if *interaction == Interaction::Pressed {
                state.selected = row.0;
                choose = true;
                break;
            }
        }
        if mouse.just_pressed(MouseButton::Left) {
            if let Some(p) = window.cursor_position() {
                let b = state.bounds;
                if p.x < b[0] || p.y < b[1] || p.x > b[0] + b[2] || p.y > b[1] + b[3] {
                    close = true;
                }
            }
        }
        if let Some((_, style, _, _, _)) = state.source.and_then(|id| selectors.get(id).ok()) {
            for (row, interaction, mut bg) in &mut rows {
                bg.0 = if *interaction == Interaction::Hovered {
                    style.hover
                } else if row.0 == state.selected {
                    style.selected
                } else {
                    style.normal
                };
            }
        }
        if choose && !close {
            if let Some(value) = state.options.get(state.selected) {
                if let Some(input) = state.source.and_then(|e| locals.get(e).ok()) {
                    local_controls::set(&mut menus, input, value.clone().into());
                } else {
                    settings.language = value.clone();
                    persistent.data.language = Some(value.clone());
                    if let Err(error) = persistent.manager.save(&persistent.data) {
                        error!("[menus] Langue non enregistrée : {error}");
                    }
                }
            }
            close = true;
        }
        if close {
            for root in &roots {
                commands.entity(root).despawn_recursive();
            }
            state.open = false;
            state.source = None;
        }
        return;
    }
    let Some((entity, appearance, _, node, transform)) = selectors
        .iter()
        .find(|(_, _, interaction, _, _)| **interaction == Interaction::Pressed)
    else {
        return;
    };
    let (options, current) = if let Ok(input) = locals.get(entity) {
        (
            input.control.options.clone(),
            input
                .control
                .value(input.kind, &menus.session)
                .as_str()
                .unwrap_or_default()
                .to_string(),
        )
    } else {
        let mut options = locales.available_langs.clone();
        if !options.contains(&settings.language) {
            options.push(settings.language.clone());
        }
        options.sort();
        options.dedup();
        (options, settings.language.clone())
    };
    if options.is_empty() {
        return;
    }
    let width = node.size().x.max(180.0).min(window.width());
    let height = (options.len() as f32 * 42.0).min(window.height() * 0.65);
    let x = (transform.translation().x - node.size().x * 0.5)
        .clamp(0.0, (window.width() - width).max(0.0));
    let below = transform.translation().y + node.size().y * 0.5;
    let y = if below + height <= window.height() {
        below
    } else {
        (transform.translation().y - node.size().y * 0.5 - height).max(0.0)
    };
    state.selected = options.iter().position(|s| s == &current).unwrap_or(0);
    state.bounds = [x, y, width, height];
    state.options = options;
    state.source = Some(entity);
    state.open = true;
    state.reveal = true;
    let root = commands
        .spawn((
            Popup,
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                z_index: ZIndex::Global(3500),
                focus_policy: bevy::ui::FocusPolicy::Block,
                ..default()
            },
        ))
        .id();
    let list = commands
        .spawn((
            PopupList,
            appearance.scrollbar.clone(),
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x),
                    top: Val::Px(y),
                    width: Val::Px(width),
                    height: Val::Px(height),
                    overflow: Overflow::clip(),
                    ..default()
                },
                background_color: appearance.normal.into(),
                ..default()
            },
            MenuScroll {
                offset: 0.0,
                content_height: state.options.len() as f32 * 42.0,
            },
        ))
        .id();
    commands.entity(root).add_child(list);
    for (i, language) in state.options.iter().enumerate() {
        commands.entity(list).with_children(|p| {
            p.spawn((
                ButtonBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(i as f32 * 42.0),
                        width: Val::Percent(100.0),
                        height: Val::Px(42.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        ..default()
                    },
                    background_color: if i == state.selected {
                        appearance.selected
                    } else {
                        appearance.normal
                    }
                    .into(),
                    ..default()
                },
                OptionRow(i),
            ))
            .with_children(|p| {
                p.spawn(TextBundle::from_section(
                    language,
                    TextStyle {
                        font: appearance.font.clone(),
                        font_size: appearance.font_size,
                        color: appearance.foreground,
                    },
                ));
            });
        });
    }
}
