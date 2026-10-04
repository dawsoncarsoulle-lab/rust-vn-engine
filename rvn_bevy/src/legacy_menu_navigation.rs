//! Adapt the built-in presentation to the shared menu navigation. Its own
//! action handlers remain authoritative; background panels keep their paint.
use super::*;

#[derive(Component)]
pub(crate) struct Covered;
#[derive(Component)]
pub(crate) struct Focusable;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ProjectionSet;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Title,
    Pause,
    Save,
    Settings,
    Gallery,
    History,
}
impl Role {
    fn id(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Pause => "pause",
            Self::Save => "save",
            Self::Settings => "settings",
            Self::Gallery => "gallery",
            Self::History => "history",
        }
    }
}
#[derive(Resource, Default)]
pub(super) struct Navigation {
    role: Option<Role>,
    settings_from_title: bool,
}
type Roots<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static TitleOverlay>,
        Option<&'static MenuOverlay>,
        Option<&'static SaveMenuOverlay>,
        Option<&'static SettingsMenuOverlay>,
        Option<&'static GalleryOverlay>,
        Option<&'static HistoryOverlay>,
        Option<&'static Covered>,
        Option<&'static crate::systems::save_menu::BuiltinSaveSnapshot>,
    ),
    OldMenus,
>;
type Hierarchy<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static Parent>,
        Option<&'static Style>,
        Option<&'static Visibility>,
    ),
>;
fn root_role(
    root: &(
        Entity,
        Option<&TitleOverlay>,
        Option<&MenuOverlay>,
        Option<&SaveMenuOverlay>,
        Option<&SettingsMenuOverlay>,
        Option<&GalleryOverlay>,
        Option<&HistoryOverlay>,
        Option<&Covered>,
        Option<&crate::systems::save_menu::BuiltinSaveSnapshot>,
    ),
) -> Role {
    if root.1.is_some() {
        Role::Title
    } else if root.2.is_some() {
        Role::Pause
    } else if root.3.is_some() {
        Role::Save
    } else if root.4.is_some() {
        Role::Settings
    } else if root.5.is_some() {
        Role::Gallery
    } else {
        Role::History
    }
}
fn owner(mut entity: Entity, roots: &Roots, hierarchy: &Hierarchy) -> Option<(Role, bool)> {
    for _ in 0..256 {
        let (parent, style, visibility) = hierarchy.get(entity).ok()?;
        if style.is_some_and(|style| style.display == Display::None)
            || visibility.is_some_and(|visibility| *visibility == Visibility::Hidden)
        {
            return None;
        }
        if let Ok(root) = roots.get(entity) {
            return Some((root_role(&root), root.8.is_some()));
        }
        entity = parent?.get();
    }
    None
}
fn current_role(
    state: &VnState,
    save: &SaveMenuState,
    settings: &SettingsMenuState,
) -> Option<Role> {
    if save.active {
        Some(Role::Save)
    } else if settings.active {
        Some(Role::Settings)
    } else {
        match state {
            VnState::TitleScreen => Some(Role::Title),
            VnState::Menu => Some(Role::Pause),
            VnState::Gallery => Some(Role::Gallery),
            VnState::History => Some(Role::History),
            _ => None,
        }
    }
}
pub(super) fn project(
    mut commands: Commands,
    state: Res<State<VnState>>,
    save: Res<SaveMenuState>,
    settings: Res<SettingsMenuState>,
    confirmation: Res<crate::systems::save_menu::SaveConfirmation>,
    screens: Option<Res<crate::programmable_ui::Screens>>,
    accessibility: Option<Res<crate::accessibility::Accessibility>>,
    mut menus: ResMut<Menus>,
    menu: Res<MenuState>,
    roots: Roots,
    hierarchy: Hierarchy,
    buttons: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&Focusable>,
            Option<&MenuFocus>,
            Option<&crate::systems::save_menu::BuiltinSlotUnavailable>,
            Option<&SaveSlotButton>,
            Option<&SaveMenuCancelButton>,
        ),
        (With<Button>, Without<MenuElement>, Without<MenuProxy>),
    >,
    mut navigation: ResMut<Navigation>,
) {
    let blocked = menus.active_page.is_some()
        || confirmation.active()
        || screens.as_ref().is_some_and(|screens| screens.modal())
        || accessibility
            .as_ref()
            .is_some_and(|accessibility| accessibility.open || accessibility.blocked);
    let role = if blocked {
        None
    } else {
        current_role(state.get(), &save, &settings)
    };
    navigation.role = role;
    navigation.settings_from_title =
        role == Some(Role::Settings) && menu.return_to == Some(VnState::TitleScreen);
    for root in &roots {
        if Some(root_role(&root)) == role {
            if root.7.is_some() {
                commands.entity(root.0).remove::<Covered>();
            }
        } else if root.7.is_none() {
            commands.entity(root.0).insert(Covered);
        }
    }
    let mut eligible = Vec::new();
    for (entity, transform, tag, _, unavailable, slot, cancel) in &buttons {
        // Preserve the no-presenter fast path: custom button ancestors are
        // not walked when authored/source/gameplay owns the input.
        let owned = (role.is_some() && unavailable.is_none())
            .then(|| owner(entity, &roots, &hierarchy))
            .flatten();
        if let Some((owned_role, native_save)) =
            owned.filter(|(owned_role, _)| Some(*owned_role) == role)
        {
            let p = transform.translation();
            let key = if owned_role == Role::Save && native_save {
                if let Some(slot) = slot {
                    format!("__builtin/save/slot/{}", slot.0)
                } else if cancel.is_some() {
                    "__builtin/save/cancel".into()
                } else {
                    format!("__builtin/save/{}", entity.to_bits())
                }
            } else {
                format!("__builtin/{}/{}", owned_role.id(), entity.to_bits())
            };
            eligible.push((entity, p.y, p.x, key));
        } else if tag.is_some() {
            commands
                .entity(entity)
                .remove::<(Focusable, MenuFocus, FocusAppearance, Outline)>();
            commands.entity(entity).insert(Interaction::None);
            if menus.focus == Some(entity) {
                menus.focus = None;
                menus.focus_key = None;
            }
            if menus.pressed == Some(entity) {
                menus.pressed = None;
            }
        }
    }
    eligible.sort_by(|a, b| {
        a.1.total_cmp(&b.1)
            .then(a.2.total_cmp(&b.2))
            .then(a.0.to_bits().cmp(&b.0.to_bits()))
    });
    for (order, (entity, _, _, key)) in eligible.into_iter().enumerate() {
        if buttons
            .get(entity)
            .is_ok_and(|(_, _, tag, focus, _, _, _)| {
                tag.is_some()
                    && focus.is_some_and(|focus| focus.0 == order as i32 && focus.1 == key)
            })
        {
            continue;
        }
        commands.entity(entity).insert((
            Focusable,
            MenuFocus(order as i32, key),
            FocusAppearance(Color::srgb(0.2, 0.7, 1.0)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::keyboard::{keyboard_input_system, Key, KeyboardFocusLost, KeyboardInput};
    use bevy::input::ButtonState;
    struct Fixture {
        app: App,
        window: Entity,
        temporary: std::path::PathBuf,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.temporary);
        }
    }
    fn visible_fixture_nodes(mut nodes: Query<&mut InheritedVisibility>) {
        for mut visible in &mut nodes {
            *visible = InheritedVisibility::VISIBLE;
        }
    }
    fn fixture(state: VnState) -> Fixture {
        let temporary = std::env::temp_dir().join(format!(
            "rvn-built-in-focus-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Image>()
            .init_asset::<Font>()
            .init_asset::<bevy::render::render_resource::Shader>()
            .init_resource::<bevy::render::camera::ManualTextureViews>()
            .add_event::<bevy::window::WindowResized>()
            .add_event::<bevy::window::WindowCreated>()
            .add_event::<bevy::window::WindowScaleFactorChanged>()
            .add_plugins(bevy::ui::UiPlugin);
        let mut schedules = app
            .world_mut()
            .resource_mut::<bevy::ecs::schedule::Schedules>();
        schedules.remove(PreUpdate);
        schedules.remove(PostUpdate);
        drop(schedules);
        app.insert_resource(State::new(state))
            .init_resource::<NextState<VnState>>()
            .init_resource::<Menus>()
            .init_resource::<Navigation>()
            .init_resource::<dropdown::Dropdown>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<GamepadButton>>()
            .init_resource::<crate::systems::save_menu::SaveConfirmation>()
            .init_resource::<SaveMenuState>()
            .init_resource::<SettingsMenuState>()
            .init_resource::<crate::systems::settings_menu::Settings>()
            .init_resource::<MenuState>()
            .init_resource::<Theme>()
            .init_resource::<VnRenderState>()
            .init_resource::<ImagemapState>()
            .init_resource::<TypewriterState>()
            .init_resource::<DialogueHistory>()
            .init_resource::<GalleryState>()
            .init_resource::<CgAssetRegistry>()
            .init_resource::<ChoiceFocus>()
            .init_resource::<crate::save_thumbnails::SaveThumbnails>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<MusicEntity>()
            .init_resource::<MusicPlaybackState>()
            .init_resource::<MusicAssetRegistry>()
            .init_resource::<MusicVolume>()
            .init_resource::<crate::accessibility::Accessibility>()
            .init_resource::<crate::programmable_ui::Screens>()
            .init_resource::<bevy::a11y::Focus>()
            .insert_resource(ProjectTitle("Test autonome".into()))
            .insert_resource(LocaleConfig {
                available_langs: vec!["fr".into(), "en".into()],
                last_modified_current: std::time::UNIX_EPOCH,
                last_modified_default: std::time::UNIX_EPOCH,
            })
            .insert_resource(ProjectPaths::new(
                temporary.clone(),
                temporary.join("assets"),
                temporary.join("locales"),
                temporary.join("theme.toml"),
                temporary.join("saves"),
            ))
            .insert_resource(PersistentDataResource {
                manager: rvn_core::PersistentDataManager::new(temporary.join("saves")).unwrap(),
                data: rvn_core::PersistentData::default(),
            })
            .insert_resource(VnEngine(
                rvn_core::Engine::new(
                    rvn_parser::parse("label start\n\"Dialogue\"\n").unwrap(),
                    crate::bevy_renderer::BevyRenderer::new(),
                    64,
                )
                .unwrap(),
            ))
            .add_event::<KeyboardInput>()
            .add_event::<KeyboardFocusLost>()
            .add_event::<AppExit>()
            .add_event::<crate::vn_command::VnCommand>()
            .add_event::<crate::vn_command::PlayerInput>()
            .add_systems(PreUpdate, keyboard_input_system)
            .add_systems(Update, (project, super::super::keyboard, back_key).chain())
            .add_systems(
                PostUpdate,
                super::super::focus_visuals.before(bevy::ui::ui_layout_system),
            )
            .add_systems(
                PostUpdate,
                (
                    bevy::render::camera::camera_system::<OrthographicProjection>,
                    bevy::ui::ui_layout_system,
                    bevy::transform::systems::sync_simple_transforms,
                    bevy::transform::systems::propagate_transforms,
                    visible_fixture_nodes,
                )
                    .chain()
                    .before(crate::accessibility::SemanticProjection),
            );
        crate::accessibility::install_semantics(&mut app);
        let window = app
            .world_mut()
            .spawn((Window::default(), bevy::window::PrimaryWindow))
            .id();
        app.world_mut().spawn(Camera2dBundle::default());
        Fixture {
            app,
            window,
            temporary,
        }
    }
    fn key(f: &mut Fixture, code: KeyCode, shift: bool) {
        if shift {
            f.app.world_mut().send_event(KeyboardInput {
                key_code: KeyCode::ShiftLeft,
                logical_key: Key::Shift,
                state: ButtonState::Pressed,
                window: f.window,
            });
        }
        f.app.world_mut().send_event(KeyboardInput {
            key_code: code,
            logical_key: match code {
                KeyCode::Tab => Key::Tab,
                KeyCode::Enter => Key::Enter,
                KeyCode::Escape => Key::Escape,
                _ => Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            },
            state: ButtonState::Pressed,
            window: f.window,
        });
        f.app.update();
        f.app.world_mut().send_event(KeyboardInput {
            key_code: code,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state: ButtonState::Released,
            window: f.window,
        });
        if shift {
            f.app.world_mut().send_event(KeyboardInput {
                key_code: KeyCode::ShiftLeft,
                logical_key: Key::Shift,
                state: ButtonState::Released,
                window: f.window,
            });
        }
        f.app.update();
    }
    fn focus<T: Component>(f: &Fixture) -> &T {
        let entity = f
            .app
            .world()
            .resource::<Menus>()
            .focus
            .expect("real keyboard focus");
        f.app.world().get::<T>(entity).unwrap()
    }
    fn ordered_key_edges(f: &mut Fixture, edges: &[(KeyCode, ButtonState)]) {
        for &(code, state) in edges {
            f.app.world_mut().send_event(KeyboardInput {
                key_code: code,
                logical_key: match code {
                    KeyCode::Tab => Key::Tab,
                    KeyCode::ShiftLeft | KeyCode::ShiftRight => Key::Shift,
                    _ => Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
                },
                state,
                window: f.window,
            });
        }
        f.app.update();
    }
    #[test]
    fn native_settings_short_shift_tab_uses_keydown_order_instead_of_released_final_state() {
        use ButtonState::{Pressed, Released};
        for shift in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
            let mut f = fixture(VnState::Menu);
            f.app.world_mut().resource_mut::<SettingsMenuState>().active = true;
            f.app.add_systems(
                Startup,
                (
                    crate::systems::spawn_menu_overlay,
                    crate::systems::spawn_settings_menu_overlay,
                ),
            );
            f.app.update();
            f.app.update();
            ordered_key_edges(&mut f, &[(KeyCode::Tab, Pressed), (KeyCode::Tab, Released)]);
            assert_eq!(*focus::<SettingsButton>(&f), SettingsButton::MusicDown);
            ordered_key_edges(
                &mut f,
                &[
                    (shift, Pressed),
                    (KeyCode::Tab, Pressed),
                    (KeyCode::Tab, Released),
                    (shift, Released),
                ],
            );
            assert_eq!(*focus::<SettingsButton>(&f), SettingsButton::Close);
            assert!(!f
                .app
                .world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(shift));
            let target = f.app.world().resource::<Menus>().focus.unwrap();
            assert!(f.app.world().get::<Outline>(target).is_some());
            ordered_key_edges(&mut f, &[(KeyCode::Tab, Pressed), (KeyCode::Tab, Released)]);
            assert_eq!(*focus::<SettingsButton>(&f), SettingsButton::MusicDown);
            // A modifier released before Tab must not reverse that later key.
            ordered_key_edges(
                &mut f,
                &[
                    (shift, Pressed),
                    (shift, Released),
                    (KeyCode::Tab, Pressed),
                    (KeyCode::Tab, Released),
                ],
            );
            assert_eq!(*focus::<SettingsButton>(&f), SettingsButton::MusicUp);
            // Nor can a later Shift press rewrite an earlier Tab keydown.
            ordered_key_edges(
                &mut f,
                &[
                    (KeyCode::Tab, Pressed),
                    (shift, Pressed),
                    (KeyCode::Tab, Released),
                    (shift, Released),
                ],
            );
            assert_eq!(*focus::<SettingsButton>(&f), SettingsButton::SfxDown);
            ordered_key_edges(&mut f, &[(shift, Pressed)]);
            ordered_key_edges(&mut f, &[(KeyCode::Tab, Pressed), (KeyCode::Tab, Released)]);
            assert_eq!(*focus::<SettingsButton>(&f), SettingsButton::MusicUp);
            ordered_key_edges(
                &mut f,
                &[
                    (shift, Released),
                    (KeyCode::Tab, Pressed),
                    (KeyCode::Tab, Released),
                ],
            );
            assert_eq!(*focus::<SettingsButton>(&f), SettingsButton::SfxDown);
        }
    }
    #[test]
    fn real_title_spawns_follow_configured_order_and_native_keys_activate_existing_handler_once() {
        let mut f = fixture(VnState::TitleScreen);
        f.app
            .world_mut()
            .resource_mut::<Theme>()
            .title_screen
            .button_order = vec!["settings".into(), "new_game".into(), "quit".into()];
        f.app
            .add_systems(Startup, crate::systems::spawn_title_screen);
        f.app.add_systems(
            Update,
            crate::systems::title_interaction_system.after(back_key),
        );
        f.app.update();
        f.app.update();
        assert!(f.app.world().resource::<Menus>().focus.is_none());
        key(&mut f, KeyCode::Tab, false);
        assert!(*focus::<TitleButton>(&f) == TitleButton::Settings);
        key(&mut f, KeyCode::Tab, true);
        assert!(*focus::<TitleButton>(&f) == TitleButton::Quit);
        key(&mut f, KeyCode::Tab, false);
        assert!(*focus::<TitleButton>(&f) == TitleButton::Settings);
        let settings_button = f.app.world().resource::<Menus>().focus.unwrap();
        key(&mut f, KeyCode::Enter, false);
        assert!(f.app.world().resource::<SettingsMenuState>().active);
        assert_eq!(
            f.app.world().resource::<MenuState>().return_to,
            Some(VnState::TitleScreen)
        );
        assert!(matches!(
            &*f.app.world().resource::<NextState<VnState>>(),
            NextState::Pending(VnState::Menu)
        ));
        assert!(f
            .app
            .world()
            .get::<Interaction>(settings_button)
            .is_some_and(|interaction| *interaction != Interaction::Pressed));
    }
    #[test]
    fn actual_settings_overlay_owns_tabs_and_native_back_returns_to_its_title_origin() {
        let mut f = fixture(VnState::Menu);
        f.app.world_mut().resource_mut::<SettingsMenuState>().active = true;
        f.app.world_mut().resource_mut::<MenuState>().return_to = Some(VnState::TitleScreen);
        f.app.add_systems(
            Startup,
            (
                crate::systems::spawn_menu_overlay,
                crate::systems::spawn_settings_menu_overlay,
            ),
        );
        f.app.add_systems(
            Update,
            (
                crate::systems::settings_menu_interaction_system.after(back_key),
                finish_settings_back.after(crate::systems::settings_menu_interaction_system),
            ),
        );
        f.app
            .add_systems(Update, crate::systems::menu_input_system.after(back_key));
        f.app.update();
        f.app.update();
        let mut pause = f
            .app
            .world_mut()
            .query_filtered::<Entity, With<MenuButton>>();
        let pause: Vec<_> = pause.iter(f.app.world()).collect();
        assert_eq!(pause.len(), 5);
        for entity in pause {
            assert!(f.app.world().get::<MenuFocus>(entity).is_none());
            assert!(f
                .app
                .world()
                .get::<bevy::a11y::AccessibilityNode>(entity)
                .is_none());
        }
        key(&mut f, KeyCode::Tab, false);
        assert!(f
            .app
            .world()
            .get::<SettingsButton>(f.app.world().resource::<Menus>().focus.unwrap())
            .is_some());
        let before = f
            .app
            .world()
            .resource::<crate::systems::settings_menu::Settings>()
            .text_speed;
        // Existing rows are located by real typed controls, never label strings.
        for _ in 0..20 {
            if *focus::<SettingsButton>(&f) == SettingsButton::TextUp {
                break;
            }
            key(&mut f, KeyCode::Tab, false);
        }
        assert!(*focus::<SettingsButton>(&f) == SettingsButton::TextUp);
        key(&mut f, KeyCode::Enter, false);
        assert!(
            (f.app
                .world()
                .resource::<crate::systems::settings_menu::Settings>()
                .text_speed
                - before
                - 0.1)
                .abs()
                < 0.0001
        );
        key(&mut f, KeyCode::Escape, false);
        assert!(!f.app.world().resource::<SettingsMenuState>().active);
        assert!(matches!(
            &*f.app.world().resource::<NextState<VnState>>(),
            NextState::Pending(VnState::TitleScreen)
        ));
        assert_eq!(f.app.world().resource::<MenuState>().return_to, None);
        assert!(f
            .app
            .world()
            .resource::<Events<crate::vn_command::PlayerInput>>()
            .is_empty());
    }
    #[test]
    fn covered_pause_receives_no_focus_and_doc_or_source_modal_keeps_its_existing_navigation() {
        use rvn_ui::programmable::{Component, ScreenView};
        let mut f = fixture(VnState::Menu);
        f.app
            .add_systems(Startup, crate::systems::spawn_menu_overlay);
        f.app
            .world_mut()
            .resource_mut::<VnRenderState>()
            .choice_options = vec!["Choix narratif derrière pause".into()];
        f.app.update();
        f.app.update();
        key(&mut f, KeyCode::Tab, false);
        assert!(*focus::<MenuButton>(&f) == MenuButton::Resume);
        let pause = f.app.world().resource::<Menus>().focus.unwrap();
        f.app.world_mut().resource_mut::<SaveMenuState>().active = true;
        f.app.update();
        assert!(f.app.world().get::<MenuFocus>(pause).is_none());
        assert!(f
            .app
            .world()
            .get::<bevy::a11y::AccessibilityNode>(pause)
            .is_none());
        f.app.world_mut().resource_mut::<SaveMenuState>().active = false;
        f.app.world_mut().resource_mut::<Menus>().active_page = Some("custom".into());
        let custom = f
            .app
            .world_mut()
            .spawn((
                ButtonBundle {
                    style: Style {
                        width: Val::Px(200.0),
                        height: Val::Px(40.0),
                        ..default()
                    },
                    ..default()
                },
                MenuFocus(0, "custom/action".into()),
            ))
            .id();
        f.app.update();
        key(&mut f, KeyCode::Tab, false);
        assert_eq!(f.app.world().resource::<Menus>().focus, Some(custom));
        key(&mut f, KeyCode::Enter, false);
        assert!(f.app.world().get::<MenuFocus>(custom).is_some());
        assert!(f.app.world().get::<Focusable>(custom).is_none());
        f.app.world_mut().despawn(custom);
        f.app.world_mut().resource_mut::<Menus>().active_page = None;
        f.app
            .world_mut()
            .resource_mut::<crate::programmable_ui::Screens>()
            .views = vec![ScreenView {
            name: "real_source_modal".into(),
            modal: true,
            layer: 0,
            order: 0,
            focus: None,
            root: Component::parse(serde_json::json!({"id":"root","kind":"column"})).unwrap(),
        }];
        f.app.update();
        assert!(f.app.world().resource::<Menus>().focus.is_none());
        assert!(f.app.world().get::<MenuFocus>(pause).is_none());
        f.app
            .world_mut()
            .resource_mut::<crate::programmable_ui::Screens>()
            .views
            .clear();
        f.app.update();
        key(&mut f, KeyCode::Tab, false);
        assert!(*focus::<MenuButton>(&f) == MenuButton::Resume);
    }
    #[test]
    fn real_save_and_load_panels_tab_to_cancel_and_escape_keeps_their_actual_origin() {
        use crate::systems::save_menu::SaveMenuOrigin;
        for (mode, origin) in [
            (SaveMenuMode::Load, SaveMenuOrigin::TitleScreen),
            (SaveMenuMode::Save, SaveMenuOrigin::InGame),
        ] {
            let mut f = fixture(VnState::Menu);
            f.app
                .world_mut()
                .resource_mut::<SaveMenuState>()
                .open(mode, origin);
            f.app.world_mut().resource_mut::<MenuState>().return_to = Some(VnState::Waiting);
            f.app.add_systems(
                Startup,
                (
                    crate::systems::spawn_menu_overlay,
                    crate::systems::spawn_save_menu_overlay,
                ),
            );
            f.app.add_systems(
                Update,
                crate::systems::save_menu_interaction_system.after(back_key),
            );
            f.app
                .add_systems(Update, crate::systems::menu_input_system.after(back_key));
            f.app.update();
            f.app.update();
            key(&mut f, KeyCode::Tab, false);
            if mode == SaveMenuMode::Save {
                assert!(f
                    .app
                    .world()
                    .get::<SaveSlotButton>(f.app.world().resource::<Menus>().focus.unwrap())
                    .is_some());
            } else {
                assert!(f
                    .app
                    .world()
                    .get::<SaveMenuCancelButton>(f.app.world().resource::<Menus>().focus.unwrap())
                    .is_some());
            }
            key(&mut f, KeyCode::Tab, true);
            assert!(f
                .app
                .world()
                .get::<SaveMenuCancelButton>(f.app.world().resource::<Menus>().focus.unwrap())
                .is_some());
            let pc = f.app.world().resource::<VnEngine>().0.state.pc;
            key(&mut f, KeyCode::Escape, false);
            assert!(!f.app.world().resource::<SaveMenuState>().active);
            assert_eq!(f.app.world().resource::<VnEngine>().0.state.pc, pc);
            assert!(f
                .app
                .world()
                .resource::<Events<crate::vn_command::PlayerInput>>()
                .is_empty());
            if origin == SaveMenuOrigin::TitleScreen {
                assert!(matches!(
                    &*f.app.world().resource::<NextState<VnState>>(),
                    NextState::Pending(VnState::TitleScreen)
                ));
                assert_eq!(f.app.world().resource::<MenuState>().return_to, None);
            } else {
                assert!(matches!(
                    &*f.app.world().resource::<NextState<VnState>>(),
                    NextState::Unchanged
                ));
                assert_eq!(
                    f.app.world().resource::<MenuState>().return_to,
                    Some(VnState::Waiting)
                );
                key(&mut f, KeyCode::Tab, false);
                assert!(*focus::<MenuButton>(&f) == MenuButton::Resume);
            }
            assert!(std::fs::read_dir(f.temporary.join("saves"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("slot_")));
        }
    }
    fn add_real_save_panel(f: &mut Fixture) {
        f.app.add_systems(
            Startup,
            (
                crate::systems::spawn_menu_overlay,
                crate::systems::spawn_save_menu_overlay,
            ),
        );
        f.app.add_systems(
            Update,
            crate::systems::save_menu_interaction_system.after(back_key),
        );
        f.app.add_systems(
            Update,
            crate::systems::spawn_save_menu_overlay.before(project),
        );
        f.app.add_systems(
            Update,
            crate::systems::despawn_save_menu_overlay
                .after(crate::systems::save_menu_interaction_system),
        );
    }
    fn native_save_root(f: &mut Fixture) -> Entity {
        let mut query = f
            .app
            .world_mut()
            .query_filtered::<Entity, With<SaveMenuOverlay>>();
        query.single(f.app.world())
    }
    fn native_slot(f: &mut Fixture, slot: usize) -> Entity {
        let mut query = f.app.world_mut().query::<(Entity, &SaveSlotButton)>();
        query
            .iter(f.app.world())
            .find(|(_, button)| button.0 == slot)
            .unwrap()
            .0
    }
    #[test]
    fn actual_approved_delete_refreshes_native_slot_and_rejects_old_default_without_touching_proxy_or_origin(
    ) {
        use crate::systems::save_menu::{
            BuiltinSlotUnavailable, SaveConfirmation, SaveMenuOrigin, SaveSlotMode,
        };
        use bevy::a11y::{accesskit, ActionRequest};
        for origin in [SaveMenuOrigin::TitleScreen, SaveMenuOrigin::InGame] {
            let mut f = fixture(VnState::Menu);
            let manager = rvn_core::save::SaveManager::new(
                f.temporary.join("saves"),
                crate::systems::save_menu::SUPPORTED_SLOTS,
            )
            .unwrap();
            f.app
                .world()
                .resource::<VnEngine>()
                .0
                .save(&manager, 2, "Only occupied slot".into(), "main.rvn".into())
                .unwrap();
            let path = f.temporary.join("saves/slot_02.json");
            let original = std::fs::read(&path).unwrap();
            f.app
                .world_mut()
                .resource_mut::<SaveMenuState>()
                .open(SaveMenuMode::Delete, origin);
            let return_to = if origin == SaveMenuOrigin::TitleScreen {
                VnState::TitleScreen
            } else {
                VnState::Waiting
            };
            f.app.world_mut().resource_mut::<MenuState>().return_to = Some(return_to.clone());
            add_real_save_panel(&mut f);
            crate::accessibility::install_assistive_actions_fixture(&mut f.app);
            f.app.update();
            f.app.update();
            let initial = native_save_root(&mut f);
            for _ in 0..3 {
                f.app.update();
                assert_eq!(native_save_root(&mut f), initial);
            }
            key(&mut f, KeyCode::Tab, false);
            assert_eq!(focus::<SaveSlotButton>(&f).0, 2);
            let old_target = f.app.world().resource::<Menus>().focus.unwrap();
            let before =
                serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap();
            key(&mut f, KeyCode::Enter, false);
            assert_eq!(
                f.app.world().resource::<SaveConfirmation>().pending,
                Some((2, SaveMenuMode::Delete))
            );
            assert_eq!(std::fs::read(&path).unwrap(), original);
            let operation = f
                .app
                .world_mut()
                .resource_mut::<SaveConfirmation>()
                .pending
                .take()
                .unwrap();
            assert_eq!(operation, (2, SaveMenuMode::Delete));
            f.app
                .world_mut()
                .resource_mut::<SaveConfirmation>()
                .approved = Some(operation);
            // The actual confirmation transport creates this exact transient
            // proxy. Native presentation rebuilding must leave it untouched.
            let proxy = f
                .app
                .world_mut()
                .spawn((
                    MenuProxy,
                    SaveSlotButton(operation.0),
                    SaveSlotMode(operation.1),
                    Interaction::Pressed,
                    BackgroundColor::default(),
                ))
                .id();
            f.app.update();
            assert!(!path.exists());
            assert_eq!(f.app.world().resource::<SaveMenuState>().revision, 1);
            assert!(f.app.world().resource::<SaveMenuState>().active);
            assert!(f
                .app
                .world()
                .resource::<SaveConfirmation>()
                .approved
                .is_none());
            f.app.update();
            let refreshed = native_save_root(&mut f);
            assert_ne!(refreshed, initial);
            assert!(f.app.world().get_entity(old_target).is_none());
            assert!(f.app.world().get_entity(proxy).is_some());
            assert!(f.app.world().get::<BuiltinSlotUnavailable>(proxy).is_none());
            let empty = f
                .app
                .world_mut()
                .query::<(Entity, &SaveSlotButton)>()
                .iter(f.app.world())
                .find(|(entity, button)| button.0 == 2 && *entity != proxy)
                .unwrap()
                .0;
            assert!(f.app.world().get::<BuiltinSlotUnavailable>(empty).is_some());
            let node = f
                .app
                .world()
                .get::<bevy::a11y::AccessibilityNode>(empty)
                .unwrap()
                .0
                .clone()
                .build();
            assert!(node.is_disabled());
            assert!(!node.supports_action(accesskit::Action::Default));
            for target in [old_target, empty] {
                f.app
                    .world_mut()
                    .send_event(ActionRequest(accesskit::ActionRequest {
                        action: accesskit::Action::Default,
                        target: accesskit::NodeId(target.to_bits()),
                        data: None,
                    }));
            }
            f.app.update();
            assert_eq!(f.app.world().resource::<SaveMenuState>().revision, 1);
            assert!(!f.app.world().resource::<SaveConfirmation>().active());
            assert_eq!(
                serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap(),
                before
            );
            key(&mut f, KeyCode::Tab, false);
            assert!(f
                .app
                .world()
                .get::<SaveMenuCancelButton>(f.app.world().resource::<Menus>().focus.unwrap())
                .is_some());
            key(&mut f, KeyCode::Escape, false);
            assert!(!f.app.world().resource::<SaveMenuState>().active);
            if origin == SaveMenuOrigin::TitleScreen {
                assert!(matches!(
                    &*f.app.world().resource::<NextState<VnState>>(),
                    NextState::Pending(VnState::TitleScreen)
                ));
                assert_eq!(f.app.world().resource::<MenuState>().return_to, None);
            } else {
                assert!(matches!(
                    &*f.app.world().resource::<NextState<VnState>>(),
                    NextState::Unchanged
                ));
                assert_eq!(
                    f.app.world().resource::<MenuState>().return_to,
                    Some(return_to)
                );
            }
        }
    }
    #[test]
    fn native_mode_change_refreshes_eligibility_preserves_hidden_display_and_ignores_foreign_roots()
    {
        use crate::systems::save_menu::{BuiltinSlotUnavailable, SaveMenuOrigin};
        let mut f = fixture(VnState::Menu);
        f.app
            .world_mut()
            .resource_mut::<SaveMenuState>()
            .open(SaveMenuMode::Save, SaveMenuOrigin::InGame);
        add_real_save_panel(&mut f);
        f.app.update();
        f.app.update();
        let initial = native_save_root(&mut f);
        let first = native_slot(&mut f, 1);
        assert!(f.app.world().get::<BuiltinSlotUnavailable>(first).is_none());
        f.app.world_mut().get_mut::<Style>(initial).unwrap().display = Display::None;
        f.app.world_mut().resource_mut::<SaveMenuState>().mode = SaveMenuMode::Load;
        f.app.update();
        let refreshed = native_save_root(&mut f);
        assert_ne!(refreshed, initial);
        assert_eq!(
            f.app.world().get::<Style>(refreshed).unwrap().display,
            Display::None
        );
        let slot = native_slot(&mut f, 1);
        assert!(f.app.world().get::<BuiltinSlotUnavailable>(slot).is_some());
        assert!(f
            .app
            .world()
            .get::<bevy::a11y::AccessibilityNode>(slot)
            .is_none());
        let foreign = f
            .app
            .world_mut()
            .spawn((SaveMenuOverlay, Style::default()))
            .id();
        f.app.world_mut().resource_mut::<SaveMenuState>().revision += 1;
        f.app.update();
        assert!(f.app.world().get_entity(foreign).is_some());
        assert!(f.app.world().get_entity(refreshed).is_some());
    }
    #[test]
    fn native_metadata_io_failure_keeps_prior_root_and_retries_only_after_an_explicit_revision() {
        use crate::systems::save_menu::SaveMenuOrigin;
        let mut f = fixture(VnState::Menu);
        f.app
            .world_mut()
            .resource_mut::<SaveMenuState>()
            .open(SaveMenuMode::Load, SaveMenuOrigin::InGame);
        add_real_save_panel(&mut f);
        f.app.update();
        f.app.update();
        let initial = native_save_root(&mut f);
        let blocked = f.temporary.join("not-a-directory");
        std::fs::write(&blocked, b"owned test IO refusal").unwrap();
        f.app.world_mut().resource_mut::<ProjectPaths>().saves = blocked;
        f.app.world_mut().resource_mut::<SaveMenuState>().revision += 1;
        f.app.update();
        assert_eq!(native_save_root(&mut f), initial);
        let repaired = f.temporary.join("repaired-metadata");
        f.app.world_mut().resource_mut::<ProjectPaths>().saves = repaired.clone();
        for _ in 0..3 {
            f.app.update();
            assert_eq!(native_save_root(&mut f), initial);
            assert!(!repaired.exists());
        }
        f.app.world_mut().resource_mut::<SaveMenuState>().revision += 1;
        f.app.update();
        assert!(repaired.is_dir());
        assert_ne!(native_save_root(&mut f), initial);
    }
    #[test]
    fn first_native_metadata_failure_keeps_real_cancel_and_reopens_after_repair_for_both_origins() {
        use crate::systems::save_menu::SaveMenuOrigin;
        for origin in [SaveMenuOrigin::TitleScreen, SaveMenuOrigin::InGame] {
            let mut f = fixture(VnState::Menu);
            let blocked = f.temporary.join("first-open-not-a-directory");
            std::fs::write(&blocked, b"owned first-open IO refusal").unwrap();
            f.app.world_mut().resource_mut::<ProjectPaths>().saves = blocked;
            f.app
                .world_mut()
                .resource_mut::<SaveMenuState>()
                .open(SaveMenuMode::Load, origin);
            let return_to = if origin == SaveMenuOrigin::TitleScreen {
                VnState::TitleScreen
            } else {
                VnState::Waiting
            };
            f.app.world_mut().resource_mut::<MenuState>().return_to = Some(return_to.clone());
            add_real_save_panel(&mut f);
            f.app.update();
            f.app.update();
            let error_root = native_save_root(&mut f);
            assert_eq!(
                f.app
                    .world_mut()
                    .query::<&SaveSlotButton>()
                    .iter(f.app.world())
                    .count(),
                0
            );
            let repaired = f.temporary.join("first-open-repaired");
            f.app.world_mut().resource_mut::<ProjectPaths>().saves = repaired.clone();
            for _ in 0..3 {
                f.app.update();
                assert_eq!(native_save_root(&mut f), error_root);
                assert!(
                    !repaired.exists(),
                    "unchanged error panel must not retry IO"
                );
            }
            key(&mut f, KeyCode::Tab, false);
            let cancel = f.app.world().resource::<Menus>().focus.unwrap();
            assert!(f.app.world().get::<SaveMenuCancelButton>(cancel).is_some());
            assert!(f.app.world().get::<Outline>(cancel).is_some());
            let node = f
                .app
                .world()
                .get::<bevy::a11y::AccessibilityNode>(cancel)
                .unwrap()
                .0
                .clone()
                .build();
            assert!(!node.is_disabled());
            assert!(node.supports_action(bevy::a11y::accesskit::Action::Default));
            key(&mut f, KeyCode::Escape, false);
            assert!(!f.app.world().resource::<SaveMenuState>().active);
            assert!(f.app.world().get_entity(error_root).is_none());
            if origin == SaveMenuOrigin::TitleScreen {
                assert!(matches!(
                    &*f.app.world().resource::<NextState<VnState>>(),
                    NextState::Pending(VnState::TitleScreen)
                ));
                assert_eq!(f.app.world().resource::<MenuState>().return_to, None);
            } else {
                assert!(matches!(
                    &*f.app.world().resource::<NextState<VnState>>(),
                    NextState::Unchanged
                ));
                assert_eq!(
                    f.app.world().resource::<MenuState>().return_to,
                    Some(return_to.clone())
                );
            }
            // Same metadata revision, explicitly reopened after repair.
            f.app
                .world_mut()
                .resource_mut::<SaveMenuState>()
                .open(SaveMenuMode::Load, origin);
            f.app.world_mut().resource_mut::<MenuState>().return_to = Some(return_to);
            f.app.update();
            f.app.update();
            assert!(repaired.is_dir());
            assert_ne!(native_save_root(&mut f), error_root);
            assert_eq!(
                f.app
                    .world_mut()
                    .query::<&SaveSlotButton>()
                    .iter(f.app.world())
                    .count(),
                crate::systems::save_menu::MAX_SLOTS
            );
            assert_eq!(f.app.world().resource::<SaveMenuState>().revision, 0);
            assert_eq!(f.app.world().resource::<SaveMenuState>().origin, origin);
        }
    }
    #[test]
    fn native_protection_refresh_preserves_live_slot_and_cancel_focus_without_reusing_entities() {
        use crate::systems::save_menu::{BuiltinSlotUnavailable, SaveMenuOrigin};
        let mut f = fixture(VnState::Menu);
        let manager = rvn_core::save::SaveManager::new(
            f.temporary.join("saves"),
            crate::systems::save_menu::SUPPORTED_SLOTS,
        )
        .unwrap();
        f.app
            .world()
            .resource::<VnEngine>()
            .0
            .save(
                &manager,
                2,
                "Focus remains on this slot".into(),
                "main.rvn".into(),
            )
            .unwrap();
        f.app
            .world_mut()
            .resource_mut::<SaveMenuState>()
            .open(SaveMenuMode::ToggleProtection, SaveMenuOrigin::InGame);
        add_real_save_panel(&mut f);
        f.app.update();
        f.app.update();
        key(&mut f, KeyCode::Tab, false);
        assert_eq!(focus::<SaveSlotButton>(&f).0, 2);
        let old_slot = f.app.world().resource::<Menus>().focus.unwrap();
        let old_root = native_save_root(&mut f);
        key(&mut f, KeyCode::Enter, false);
        assert!(manager.is_protected(2).unwrap());
        assert_eq!(f.app.world().resource::<SaveMenuState>().revision, 1);
        assert_ne!(native_save_root(&mut f), old_root);
        assert_eq!(focus::<SaveSlotButton>(&f).0, 2);
        let new_slot = f.app.world().resource::<Menus>().focus.unwrap();
        assert_ne!(new_slot, old_slot);
        assert!(f.app.world().get::<Outline>(new_slot).is_some());
        assert!(f.app.world().get_entity(old_slot).is_none());
        assert!(f
            .app
            .world()
            .get::<BuiltinSlotUnavailable>(new_slot)
            .is_none());
        key(&mut f, KeyCode::Tab, false);
        let old_cancel = f.app.world().resource::<Menus>().focus.unwrap();
        assert!(f
            .app
            .world()
            .get::<SaveMenuCancelButton>(old_cancel)
            .is_some());
        f.app.world_mut().resource_mut::<SaveMenuState>().revision += 1;
        f.app.update();
        let new_cancel = f.app.world().resource::<Menus>().focus.unwrap();
        assert_ne!(new_cancel, old_cancel);
        assert!(f.app.world().get::<Outline>(new_cancel).is_some());
        assert!(f
            .app
            .world()
            .get::<SaveMenuCancelButton>(new_cancel)
            .is_some());
        assert!(f.app.world().get_entity(old_cancel).is_none());
    }
    #[test]
    fn actual_empty_non_save_slots_are_disabled_skip_tab_and_ignore_pressed_pointer_actions() {
        use crate::systems::save_menu::{BuiltinSlotUnavailable, SaveMenuOrigin};
        use bevy::a11y::accesskit::Action;
        for mode in [
            SaveMenuMode::Load,
            SaveMenuMode::Delete,
            SaveMenuMode::ToggleProtection,
        ] {
            let mut f = fixture(VnState::Menu);
            f.app
                .world_mut()
                .resource_mut::<SaveMenuState>()
                .open(mode, SaveMenuOrigin::InGame);
            add_real_save_panel(&mut f);
            f.app.update();
            f.app.update();
            key(&mut f, KeyCode::Tab, false);
            assert!(f
                .app
                .world()
                .get::<SaveMenuCancelButton>(f.app.world().resource::<Menus>().focus.unwrap())
                .is_some());
            key(&mut f, KeyCode::Tab, true);
            assert!(f
                .app
                .world()
                .get::<SaveMenuCancelButton>(f.app.world().resource::<Menus>().focus.unwrap())
                .is_some());
            let mut query = f
                .app
                .world_mut()
                .query_filtered::<Entity, With<SaveSlotButton>>();
            let slots: Vec<_> = query.iter(f.app.world()).collect();
            assert_eq!(slots.len(), crate::systems::save_menu::MAX_SLOTS);
            for entity in &slots {
                assert!(f
                    .app
                    .world()
                    .get::<BuiltinSlotUnavailable>(*entity)
                    .is_some());
                assert!(f.app.world().get::<MenuFocus>(*entity).is_none());
                let node = f
                    .app
                    .world()
                    .get::<bevy::a11y::AccessibilityNode>(*entity)
                    .unwrap()
                    .0
                    .clone()
                    .build();
                assert!(node.is_disabled());
                assert!(!node.is_hidden());
                assert!(!node.supports_action(Action::Focus));
                assert!(!node.supports_action(Action::Default));
            }
            let before =
                serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap();
            for entity in slots {
                *f.app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::Pressed;
            }
            f.app.update();
            assert_eq!(
                serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap(),
                before
            );
            let save = f.app.world().resource::<SaveMenuState>();
            assert!(save.active);
            assert_eq!(save.revision, 0);
            assert!(!f
                .app
                .world()
                .resource::<crate::systems::save_menu::SaveConfirmation>()
                .active());
            assert!(matches!(
                &*f.app.world().resource::<NextState<VnState>>(),
                NextState::Unchanged
            ));
            assert!(std::fs::read_dir(f.temporary.join("saves"))
                .unwrap()
                .next()
                .is_none());
        }
    }
    #[test]
    fn actual_load_tabs_only_to_occupied_slot_two_and_native_enter_loads_that_exact_data() {
        use crate::systems::save_menu::{BuiltinSlotUnavailable, SaveMenuOrigin};
        let mut f = fixture(VnState::Menu);
        let manager = rvn_core::save::SaveManager::new(
            f.temporary.join("saves"),
            crate::systems::save_menu::SUPPORTED_SLOTS,
        )
        .unwrap();
        f.app
            .world_mut()
            .resource_mut::<VnEngine>()
            .0
            .state
            .vars
            .insert("load_probe".into(), rvn_parser::Value::Int(42));
        f.app
            .world()
            .resource::<VnEngine>()
            .0
            .save(&manager, 2, "Unique slot".into(), "main.rvn".into())
            .unwrap();
        let slot_path = f.temporary.join("saves/slot_02.json");
        let saved = std::fs::read(&slot_path).unwrap();
        f.app
            .world_mut()
            .resource_mut::<VnEngine>()
            .0
            .state
            .vars
            .insert("load_probe".into(), rvn_parser::Value::Int(999));
        f.app
            .world_mut()
            .resource_mut::<SaveMenuState>()
            .open(SaveMenuMode::Load, SaveMenuOrigin::TitleScreen);
        add_real_save_panel(&mut f);
        f.app.update();
        f.app.update();
        key(&mut f, KeyCode::Tab, false);
        assert_eq!(focus::<SaveSlotButton>(&f).0, 2);
        assert!(f
            .app
            .world()
            .get::<BuiltinSlotUnavailable>(f.app.world().resource::<Menus>().focus.unwrap())
            .is_none());
        key(&mut f, KeyCode::Tab, false);
        assert!(f
            .app
            .world()
            .get::<SaveMenuCancelButton>(f.app.world().resource::<Menus>().focus.unwrap())
            .is_some());
        key(&mut f, KeyCode::Tab, false);
        assert_eq!(focus::<SaveSlotButton>(&f).0, 2);
        key(&mut f, KeyCode::Enter, false);
        assert_eq!(
            f.app.world().resource::<VnEngine>().0.state.vars["load_probe"],
            rvn_parser::Value::Int(42)
        );
        assert!(!f.app.world().resource::<SaveMenuState>().active);
        assert!(matches!(
            &*f.app.world().resource::<NextState<VnState>>(),
            NextState::Pending(VnState::Waiting)
        ));
        assert_eq!(std::fs::read(&slot_path).unwrap(), saved);
        assert_eq!(manager.list_saves().len(), 1);
    }
    #[test]
    fn actual_empty_save_slot_remains_focusable_and_native_enter_saves_it_once() {
        use crate::systems::save_menu::{BuiltinSlotUnavailable, SaveMenuOrigin};
        use bevy::a11y::accesskit::Action;
        let mut f = fixture(VnState::Menu);
        f.app
            .world_mut()
            .resource_mut::<VnEngine>()
            .0
            .state
            .vars
            .insert("save_probe".into(), rvn_parser::Value::Int(77));
        f.app
            .world_mut()
            .resource_mut::<SaveMenuState>()
            .open(SaveMenuMode::Save, SaveMenuOrigin::InGame);
        add_real_save_panel(&mut f);
        f.app.update();
        f.app.update();
        key(&mut f, KeyCode::Tab, false);
        assert_eq!(focus::<SaveSlotButton>(&f).0, 1);
        let target = f.app.world().resource::<Menus>().focus.unwrap();
        assert!(f
            .app
            .world()
            .get::<BuiltinSlotUnavailable>(target)
            .is_none());
        let node = f
            .app
            .world()
            .get::<bevy::a11y::AccessibilityNode>(target)
            .unwrap()
            .0
            .clone()
            .build();
        assert!(!node.is_disabled());
        assert!(node.supports_action(Action::Focus));
        assert!(node.supports_action(Action::Default));
        key(&mut f, KeyCode::Enter, false);
        let manager = rvn_core::save::SaveManager::new(
            f.temporary.join("saves"),
            crate::systems::save_menu::SUPPORTED_SLOTS,
        )
        .unwrap();
        let data = manager.load(1).unwrap();
        assert_eq!(data.vars["save_probe"], rvn_core::save::SaveValue::Int(77));
        assert!(!f.app.world().resource::<SaveMenuState>().active);
        let slot_path = f.temporary.join("saves/slot_01.json");
        let bytes = std::fs::read(&slot_path).unwrap();
        f.app.update();
        f.app.update();
        assert_eq!(std::fs::read(&slot_path).unwrap(), bytes);
        assert_eq!(manager.list_saves().len(), 1);
    }
    #[test]
    fn actual_gallery_buttons_and_history_escape_preserve_the_existing_return_destinations() {
        for origin in [VnState::TitleScreen, VnState::Waiting] {
            let mut f = fixture(VnState::Gallery);
            f.app.world_mut().resource_mut::<MenuState>().return_to = Some(origin.clone());
            f.app
                .add_systems(Startup, crate::systems::spawn_gallery_overlay);
            f.app.add_systems(
                Update,
                crate::systems::gallery_interaction_system.after(back_key),
            );
            f.app.update();
            f.app.update();
            key(&mut f, KeyCode::Tab, false);
            assert!(matches!(focus::<GalleryButton>(&f), GalleryButton::ShowCg));
            key(&mut f, KeyCode::Tab, true);
            assert!(matches!(focus::<GalleryButton>(&f), GalleryButton::Back));
            key(&mut f, KeyCode::Enter, false);
            assert!(
                matches!(&*f.app.world().resource::<NextState<VnState>>(),NextState::Pending(state) if *state==origin)
            );
            assert_eq!(f.app.world().resource::<MenuState>().return_to, None);
        }
        let mut f = fixture(VnState::History);
        f.app
            .add_systems(Startup, crate::systems::spawn_history_overlay);
        f.app
            .add_systems(Update, crate::systems::history_input_system.after(back_key));
        f.app.update();
        f.app.update();
        key(&mut f, KeyCode::Tab, false);
        assert!(f.app.world().resource::<Menus>().focus.is_none());
        key(&mut f, KeyCode::Escape, false);
        assert!(matches!(
            &*f.app.world().resource::<NextState<VnState>>(),
            NextState::Pending(VnState::Waiting)
        ));
    }
    #[derive(Resource,Default)]
    struct PausePaintChanges(usize);
    fn pause_paint_changes(
        buttons: Query<(),(With<MenuButton>,Changed<BackgroundColor>)>,
        mut changes:ResMut<PausePaintChanges>,
    ) {changes.0=buttons.iter().count();}
    #[test]
    fn real_projection_repaints_covered_pause_and_return_without_an_interaction_event_stays_clean() {
        use crate::systems::save_menu::SaveMenuOrigin;
        for overlay in ["save","load","settings"] {
            let mut f=fixture(VnState::Menu);
            f.app.add_systems(Startup,crate::systems::spawn_menu_overlay)
                .init_resource::<PausePaintChanges>()
                .add_systems(Update,crate::systems::menu_interaction_system.after(back_key))
                .add_systems(Update,pause_paint_changes.after(crate::systems::menu_interaction_system));
            f.app.update();f.app.update();
            let mut query=f.app.world_mut().query_filtered::<Entity,With<MenuButton>>();
            let buttons:Vec<_>=query.iter(f.app.world()).collect();assert_eq!(buttons.len(),5);
            let narrative=serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap();
            for entity in &buttons {*f.app.world_mut().get_mut::<Interaction>(*entity).unwrap()=Interaction::Hovered;}
            f.app.update();
            for entity in &buttons {assert_eq!(f.app.world().get::<BackgroundColor>(*entity).unwrap().0,
                Color::srgba(0.15,0.15,0.35,0.95));assert!(f.app.world().get::<Focusable>(*entity).is_some());}
            if overlay=="settings" {f.app.world_mut().resource_mut::<SettingsMenuState>().active=true;}
            else {f.app.world_mut().resource_mut::<SaveMenuState>().open(
                if overlay=="load"{SaveMenuMode::Load}else{SaveMenuMode::Save},SaveMenuOrigin::InGame);}
            // project itself removes focus and resets these Interaction values;
            // the test does not inject None or manually repaint their colors.
            f.app.update();
            for entity in &buttons {
                assert_eq!(*f.app.world().get::<Interaction>(*entity).unwrap(),Interaction::None);
                assert!(f.app.world().get::<Focusable>(*entity).is_none());
                assert_eq!(f.app.world().get::<BackgroundColor>(*entity).unwrap().0,
                    Color::srgba(0.08,0.08,0.20,0.95),"covered reset must repaint under {overlay}");
            }
            assert_eq!(f.app.world().resource::<PausePaintChanges>().0,5);
            f.app.update();assert_eq!(f.app.world().resource::<PausePaintChanges>().0,0);
            f.app.world_mut().resource_mut::<SaveMenuState>().active=false;
            f.app.world_mut().resource_mut::<SettingsMenuState>().active=false;
            // No pointer or key event and no Interaction mutation accompanies
            // the return. All five buttons become eligible again, still normal.
            f.app.update();
            for entity in &buttons {
                assert!(f.app.world().get::<Focusable>(*entity).is_some());
                assert_eq!(*f.app.world().get::<Interaction>(*entity).unwrap(),Interaction::None);
                assert_eq!(f.app.world().get::<BackgroundColor>(*entity).unwrap().0,Color::srgba(0.08,0.08,0.20,0.95));
            }
            assert_eq!(f.app.world().resource::<PausePaintChanges>().0,0);
            assert_eq!(serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap(),narrative);
            assert!(f.app.world().resource::<Events<AppExit>>().is_empty());
            assert!(!f.app.world().resource::<SaveMenuState>().active);
            assert!(std::fs::read_dir(f.temporary.join("saves")).unwrap().next().is_none());
        }
    }

}
pub(super) fn back_key(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    navigation: Option<Res<Navigation>>,
    mut menus: ResMut<Menus>,
    mut buttons: Query<
        (
            Entity,
            &mut Interaction,
            Option<&SettingsButton>,
            Option<&SaveMenuCancelButton>,
        ),
        With<Focusable>,
    >,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    let role = navigation.and_then(|navigation| navigation.role);
    let target = match role {
        Some(Role::Settings) => buttons
            .iter()
            .find(|(_, _, settings, _)| {
                settings.is_some_and(|button| *button == SettingsButton::Close)
            })
            .map(|row| row.0),
        Some(Role::Save) => buttons
            .iter()
            .find(|(_, _, _, cancel)| cancel.is_some())
            .map(|row| row.0),
        _ => None,
    };
    if let Some(target) = target {
        if let Ok((_, mut interaction, _, _)) = buttons.get_mut(target) {
            *interaction = Interaction::Pressed;
            menus.pressed = Some(target);
        }
        keys.clear_just_pressed(KeyCode::Escape);
    }
}
pub(super) fn finish_settings_back(
    navigation: Option<Res<Navigation>>,
    settings: Res<SettingsMenuState>,
    mut menu: ResMut<MenuState>,
    mut next: ResMut<NextState<VnState>>,
    buttons: Query<(&Interaction, &SettingsButton), With<Focusable>>,
) {
    if navigation.is_some_and(|navigation| navigation.settings_from_title)
        && !settings.active
        && buttons.iter().any(|(interaction, button)| {
            *interaction == Interaction::Pressed && *button == SettingsButton::Close
        })
    {
        menu.return_to = None;
        next.set(VnState::TitleScreen);
    }
}
