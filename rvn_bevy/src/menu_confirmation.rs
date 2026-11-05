//! Destructive save operations have one transient approval gate.
use super::*;
use crate::systems::save_menu::SaveConfirmation;
#[path = "menu_confirmation_image.rs"]
pub(crate) mod preview_image;
#[derive(Component)]
pub(super) struct ConfirmationRoot;
#[derive(Component)]
pub(crate) struct ConfirmationButton(pub(crate) bool);
#[derive(Component)]
pub(super) struct ConfirmationColors {
    normal: Color,
    hover: Color,
    pressed: Color,
}
#[derive(Component)]
pub(super) struct ConfirmationTarget(String, Option<i32>);
fn focused_element(elements: &[Element], approve: bool) -> Option<&Element> {
    for e in elements {
        if !e.visible || !e.enabled {
            continue;
        }
        if e.action
            == if approve {
                Action::Confirm
            } else {
                Action::Back
            }
        {
            return Some(e);
        }
        if let Some(child) = focused_element(&e.children, approve) {
            return Some(child);
        }
    }
    None
}

fn available_targets(
    doc: &Document,
    page: usize,
) -> (std::collections::BTreeSet<String>, Vec<(i32, String)>) {
    let mut pointer = std::collections::BTreeSet::new();
    let mut keyboard = Vec::new();
    for e in doc.layout_page(page, doc.reference) {
        if !e.enabled {
            continue;
        }
        let events = doc.pages[page]
            .graphs
            .iter()
            .filter(|g| g.target.as_deref() == Some(e.id.as_str()))
            .map(|g| g.event.clone());
        let (p, k) = interaction_policy(e.kind.clone(), events);
        let decision = matches!(e.action, Action::Confirm | Action::Back);
        if p || decision {
            pointer.insert(e.id.clone());
        }
        if k || decision {
            keyboard.push((e.focus_order, e.id));
        }
    }
    keyboard.sort();
    (pointer, keyboard)
}

fn dispatch(
    commands: &mut Commands,
    menus: &mut Menus,
    assets: &AssetServer,
    volume: f32,
    page: &rvn_ui::Page,
    target: Option<&str>,
    event: rvn_ui::Event,
    fallback: Action,
) -> Option<bool> {
    let mut decision = None;
    match menus
        .session
        .dispatch_confirmation(page, target, event, fallback)
    {
        Err(error) => {
            error!("Confirmation : {error}");
            report_preview_error(&menus.session);
        }
        Ok(effects) => {
            for effect in effects {
                match effect {
                    Effect::Action(Action::Confirm) => decision = Some(true),
                    Effect::Action(Action::Back) => decision = Some(false),
                    Effect::Animate(id, clip) => {
                        menus.animation_requests.push((page.id.clone(), id, clip))
                    }
                    Effect::Sound(path) => {
                        commands.spawn(AudioBundle {
                            source: assets.load(path),
                            settings: bevy::audio::PlaybackSettings::DESPAWN
                                .with_volume(bevy::audio::Volume::new(volume)),
                            ..default()
                        });
                    }
                    effect => menus.session.apply_presentation(&page.id, &effect),
                }
            }
        }
    }
    decision
}

pub(super) fn update(
    mut commands: Commands,
    mut confirmation: ResMut<SaveConfirmation>,
    mut menus: ResMut<Menus>,
    ctx: Context,
    assets: Res<AssetServer>,
    roots: Query<Entity, With<ConfirmationRoot>>,
    mut buttons: Query<(
        Ref<Interaction>,
        Option<&ConfirmationButton>,
        &ConfirmationColors,
        &mut BackgroundColor,
        &ConfirmationTarget,
    )>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    pads: Res<ButtonInput<GamepadButton>>,
    mut previous: Local<Option<String>>,
    mut focus: Local<Option<String>>,
    mut opened: Local<Option<rvn_ui::Page>>,
    mut hovering: Local<std::collections::BTreeSet<String>>,
    mut next: ResMut<NextState<VnState>>,
    mut player: EventWriter<crate::vn_command::PlayerInput>,
    source: Option<Res<crate::source_menus::SourceMenus>>,
) {
    if source.as_deref().is_some_and(|source|source.active(rvn_ui::PageRole::Confirm)) {
        for root in &roots {commands.entity(root).despawn_recursive();}
        *previous=None; *focus=None; opened.take(); hovering.clear();
        return;
    }
    let operation = confirmation.pending;
    let action = confirmation.action_pending.clone();
    if !confirmation.active() {
        if let Some(page) = opened.take() {
            dispatch(
                &mut commands,
                &mut menus,
                &assets,
                ctx.values.sfx_volume,
                &page,
                None,
                rvn_ui::Event::Close,
                Action::None,
            );
        }
        hovering.clear();
        for root in &roots {
            commands.entity(root).despawn_recursive();
        }
        *previous = None;
        return;
    }
    let mut doc = menus.doc.clone().unwrap_or_else(Document::defaults);
    let index = doc
        .pages
        .iter()
        .position(|p| p.role == Some(rvn_ui::PageRole::Confirm))
        .unwrap_or_else(|| {
            let page = Document::defaults()
                .pages
                .into_iter()
                .find(|p| p.role == Some(rvn_ui::PageRole::Confirm))
                .unwrap();
            doc.pages.push(page);
            doc.pages.len() - 1
        });
    let opening = opened.is_none();
    if opening {
        *focus = focused_element(&doc.pages[index].elements, false).map(|e| e.id.clone());
    }
    if opening {
        *opened = Some(doc.pages[index].clone());
        dispatch(
            &mut commands,
            &mut menus,
            &assets,
            ctx.values.sfx_volume,
            &doc.pages[index],
            None,
            rvn_ui::Event::Open,
            Action::None,
        );
    }
    let next_hover: std::collections::BTreeSet<_> = buttons
        .iter()
        .filter(|(i, _, _, _, _)| **i != Interaction::None)
        .map(|(_, _, _, _, t)| t.0.clone())
        .collect();
    for id in next_hover.difference(&hovering) {
        dispatch(
            &mut commands,
            &mut menus,
            &assets,
            ctx.values.sfx_volume,
            &doc.pages[index],
            Some(id),
            rvn_ui::Event::Hover,
            Action::None,
        );
    }
    for id in hovering.difference(&next_hover) {
        dispatch(
            &mut commands,
            &mut menus,
            &assets,
            ctx.values.sfx_volume,
            &doc.pages[index],
            Some(id),
            rvn_ui::Event::HoverLeave,
            Action::None,
        );
    }
    *hovering = next_hover;
    // Presentation graphs can hide/disable the focused control during this
    // frame, before its old entity is rebuilt. Do not activate stale entities.
    let presented = menus.session.present(&doc);
    let (available, items) = available_targets(&presented, index);
    let repaired = focus
        .as_ref()
        .is_none_or(|id| !items.iter().any(|(_, item)| item == id));
    if repaired {
        *focus = focused_element(&presented.pages[index].elements, false)
            .map(|e| e.id.clone())
            .filter(|id| items.iter().any(|(_, item)| item == id))
            .or_else(|| items.first().map(|(_, id)| id.clone()));
        *previous = None;
    }
    let pad = |kind| pads.get_just_pressed().any(|p| p.button_type == kind);
    let mut activation = buttons
        .iter()
        .find(|(i, _, _, _, t)| {
            **i == Interaction::Pressed && i.is_changed() && available.contains(&t.0)
        })
        .map(|(_, b, _, _, target)| (b.map(|b| b.0), target.0.clone()));
    let mut decision = None;
    for (i, _, colors, mut bg, _) in &mut buttons {
        bg.0 = match *i {
            Interaction::Pressed => colors.pressed,
            Interaction::Hovered => colors.hover,
            Interaction::None => colors.normal,
        };
    }
    let moved = keys.just_pressed(KeyCode::Tab)
        || keys.just_pressed(KeyCode::ArrowUp)
        || keys.just_pressed(KeyCode::ArrowDown)
        || pad(GamepadButtonType::DPadUp)
        || pad(GamepadButtonType::DPadDown)
        || keys.just_pressed(KeyCode::ArrowLeft)
        || keys.just_pressed(KeyCode::ArrowRight)
        || pad(GamepadButtonType::DPadLeft)
        || pad(GamepadButtonType::DPadRight);
    if moved {
        if !items.is_empty() {
            let backward = keys.just_pressed(KeyCode::ArrowUp)
                || keys.just_pressed(KeyCode::ArrowLeft)
                || pad(GamepadButtonType::DPadUp)
                || pad(GamepadButtonType::DPadLeft)
                || (keys.just_pressed(KeyCode::Tab)
                    && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)));
            let current = items.iter().position(|(_, id)| focus.as_ref() == Some(id));
            let index = match current {
                Some(i) if backward => (i + items.len() - 1) % items.len(),
                Some(i) => (i + 1) % items.len(),
                None if backward => items.len() - 1,
                None => 0,
            };
            *focus = Some(items[index].1.clone());
            *previous = None;
        }
    }
    if opening || moved || repaired {
        if let Some(id) = focus.as_deref() {
            dispatch(
                &mut commands,
                &mut menus,
                &assets,
                ctx.values.sfx_volume,
                &doc.pages[index],
                Some(id),
                rvn_ui::Event::Focus,
                Action::None,
            );
        }
    }
    if keys.just_pressed(KeyCode::Escape) || pad(GamepadButtonType::East) {
        decision = Some(false);
    }
    if keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || pad(GamepadButtonType::South)
    {
        activation = buttons
            .iter()
            .find(|(_, _, _, _, t)| focus.as_ref() == Some(&t.0))
            .map(|(_, b, _, _, target)| (b.map(|b| b.0), target.0.clone()));
    }
    if let Some((_, id)) = &activation {
        if !available_targets(&menus.session.present(&doc), index)
            .0
            .contains(id)
        {
            activation = None;
        }
    }
    if decision.is_none() {
        if let Some((approved, id)) = activation {
            decision = dispatch(
                &mut commands,
                &mut menus,
                &assets,
                ctx.values.sfx_volume,
                &doc.pages[index],
                Some(&id),
                rvn_ui::Event::Click,
                match approved {
                    Some(true) => Action::Confirm,
                    Some(false) => Action::Back,
                    None => Action::None,
                },
            );
        }
    }
    for key in [
        KeyCode::Escape,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::Tab,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::Enter,
        KeyCode::Space,
    ] {
        keys.clear_just_pressed(key);
    }
    if let Some(approved) = decision {
        if let Some(page) = opened.take() {
            for id in hovering.iter() {
                dispatch(
                    &mut commands,
                    &mut menus,
                    &assets,
                    ctx.values.sfx_volume,
                    &page,
                    Some(id),
                    rvn_ui::Event::HoverLeave,
                    Action::None,
                );
            }
            dispatch(
                &mut commands,
                &mut menus,
                &assets,
                ctx.values.sfx_volume,
                &page,
                None,
                rvn_ui::Event::Close,
                Action::None,
            );
        }
        hovering.clear();
        info!("[confirmation] decision {operation:?} / {action:?}: {approved}");
        confirmation.pending = None;
        if let Some(operation) = operation {
            if operation.0 == 0 {
                let previous = confirmation.quick_return.take().unwrap_or(VnState::Waiting);
                next.set(previous);
                if approved {
                    confirmation.approved = Some(operation);
                    player.send(crate::vn_command::PlayerInput::QuickLoad);
                }
            } else if approved {
                confirmation.approved = Some(operation);
                commands.spawn((
                    MenuProxy,
                    SaveSlotButton(operation.0),
                    crate::systems::save_menu::SaveSlotMode(operation.1),
                    ButtonBundle {
                        style: Style {
                            display: Display::None,
                            ..default()
                        },
                        interaction: Interaction::Pressed,
                        ..default()
                    },
                ));
            }
        } else if let Some((_, page, origin)) = action {
            confirmation.action_pending = None;
            next.set(origin);
            // Empty element IDs are forbidden in documents: this internal
            // acknowledgement cannot re-run the source button's graph.
            if approved {
                menus.activations.push((
                    page,
                    Some(String::new()),
                    rvn_ui::Event::Click,
                    Action::Confirm,
                ));
            } else {
                menus.pending = None;
            }
        }
        for root in &roots {
            commands.entity(root).despawn_recursive();
        }
        *previous = None;
        *focus = None;
        return;
    }
    doc = menus.session.present(&doc);
    let Ok(window) = ctx.windows.get_single() else {
        return;
    };
    let size = [window.width(), window.height()];
    let key = format!(
        "{operation:?}/{action:?}/{size:?}/{}",
        serde_json::to_string(&doc.pages[index]).unwrap_or_default()
    );
    let key = format!("{key}:fonts={}", ctx.fonts.len());
    if previous.as_ref() == Some(&key) {
        return;
    }
    *previous = Some(key);
    for root in &roots {
        commands.entity(root).despawn_recursive();
    }
    let root = commands
        .spawn((
            ConfirmationRoot,
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                background_color: color(doc.pages[index].background).into(),
                z_index: ZIndex::Global(4000),
                focus_policy: bevy::ui::FocusPolicy::Block,
                ..default()
            },
        ))
        .id();
    let title = if let Some((slot, mode)) = operation {
        if slot == 0 {
            translated(
                &ctx,
                "Charger la sauvegarde rapide et remplacer la partie en cours ?",
            )
        } else if mode == SaveMenuMode::Delete {
            translated(&ctx, "Supprimer définitivement la sauvegarde [slot] ?")
                .replace("[slot]", &slot.to_string())
        } else if mode == SaveMenuMode::Save {
            translated(&ctx, "Remplacer la sauvegarde [slot] ?")
                .replace("[slot]", &slot.to_string())
        } else {
            translated(
                &ctx,
                "Charger la sauvegarde [slot] et remplacer la partie en cours ?",
            )
            .replace("[slot]", &slot.to_string())
        }
    } else if matches!(action, Some((Action::Continue, _, _))) {
        translated(
            &ctx,
            "Continuer la sauvegarde et remplacer la partie en cours ?",
        )
    } else {
        translated(
            &ctx,
            "Démarrer une nouvelle partie et remplacer la partie en cours ?",
        )
    };
    for e in menu_layout::tree(
        &doc,
        index,
        size,
        &assets,
        &ctx,
        0,
        &[("confirmation.message", &title)],
    ) {
        spawn_confirmation(
            &mut commands,
            root,
            &e,
            &doc.pages[index].id,
            size,
            &assets,
            &ctx,
            &doc,
            &title,
            focus.as_deref(),
        );
    }
}

fn spawn_confirmation(
    commands: &mut Commands,
    parent: Entity,
    e: &Element,
    page: &str,
    size: [f32; 2],
    assets: &AssetServer,
    ctx: &Context,
    doc: &Document,
    title: &str,
    focus: Option<&str>,
) {
    let button = if e.enabled {
        match e.action {
            Action::Confirm => Some(true),
            Action::Back => Some(false),
            _ => None,
        }
    } else {
        None
    };
    let mut shallow = e.clone();
    shallow.children.clear();
    shallow.action = Action::None;
    if e.kind == Kind::Text
        && (e.id == "heading" || e.binding.as_deref() == Some("confirmation.message"))
    {
        shallow.text = title.into();
        shallow.binding = None;
    }
    if focus == Some(e.id.as_str()) {
        shallow.normal = e.appearance.focus.unwrap_or(e.hover);
    }
    let Some(entity) = spawn_element(
        commands, parent, &shallow, page, size, size, assets, ctx, doc, 0,
    ) else {
        return;
    };
    commands
        .entity(entity)
        .remove::<MenuElement>()
        .remove::<MenuFocus>();
    if e.kind == Kind::Text {
        commands.entity(entity).insert(Style {
            position_type: PositionType::Absolute,
            left: Val::Px(e.rect[0]),
            top: Val::Px(e.rect[1]),
            width: Val::Px(e.rect[2]),
            height: Val::Auto,
            min_height: Val::Px(e.rect[3]),
            ..default()
        });
    }
    let events = doc
        .pages
        .iter()
        .filter(|p| p.id == page)
        .flat_map(|p| &p.graphs)
        .filter(|g| g.target.as_deref() == Some(e.id.as_str()))
        .map(|g| g.event.clone());
    let (pointer, keyboard) = interaction_policy(e.kind.clone(), events);
    if e.enabled && pointer {
        commands.entity(entity).insert((
            Interaction::None,
            ConfirmationTarget(e.id.clone(), keyboard.then_some(e.focus_order)),
            ConfirmationColors {
                normal: color(shallow.normal),
                hover: color(e.hover),
                pressed: color(e.pressed),
            },
        ));
    }
    if let Some(yes) = button {
        commands.entity(entity).insert((
            Button,
            Interaction::None,
            ConfirmationButton(yes),
            ConfirmationTarget(e.id.clone(), Some(e.focus_order)),
            ConfirmationColors {
                normal: color(shallow.normal),
                hover: color(e.hover),
                pressed: color(e.pressed),
            },
        ));
    }
    for child in &e.children {
        spawn_confirmation(
            commands,
            entity,
            child,
            page,
            [e.rect[2], e.rect[3]],
            assets,
            ctx,
            doc,
            title,
            focus,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modal_focus_excludes_hidden_and_disabled_descendants() {
        let mut doc = Document::defaults();
        let index = doc
            .pages
            .iter()
            .position(|p| p.role == Some(rvn_ui::PageRole::Confirm))
            .unwrap();
        let mut panel = Element::new("group".into(), Kind::Panel);
        let mut child = Element::new("extra".into(), Kind::Button);
        child.focus_order = -10;
        panel.children.push(child);
        doc.pages[index].elements.push(panel);
        assert_eq!(available_targets(&doc, index).1.first().unwrap().1, "extra");
        for effect in [
            Effect::Visible("group".into(), false),
            Effect::Enabled("group".into(), false),
        ] {
            let mut session = rvn_ui::Session::default();
            session.apply_presentation(&doc.pages[index].id, &effect);
            let (pointer, keyboard) = available_targets(&session.present(&doc), index);
            assert!(!pointer.contains("extra"));
            assert!(!keyboard.iter().any(|(_, id)| id == "extra"));
            assert!(!keyboard.is_empty());
        }
        assert!(doc.find_element(index, "group").unwrap().enabled);
    }
}

// Proposed test module appended to menu_documents::confirmation.
// NOT INSTALLED / NOT TYPE-CHECKED / NOT RUN. CPU geometry is explicit fixture
// data, never evidence of a completed GPU image or native pixel parity.
#[cfg(test)]
mod completed_image_legacy_confirmation_contracts {
    use super::*;
    use bevy::asset::AssetApp;
    use rvn_core::editor_preview::{FrameScope, ImageInput, PointerKind};
    use crate::vn_command::{PlayerInput, VnCommand};

    struct Fixture {
        app: App, directory: std::path::PathBuf,
        capture_sequence: u64, input_sequence: u64,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let temporary = std::env::temp_dir().canonicalize().unwrap();
            let directory = self.directory.canonicalize().unwrap();
            assert_eq!(directory.parent(), Some(temporary.as_path()));
            assert!(directory.file_name().unwrap().to_string_lossy().starts_with("rvn-image-confirmation-"));
            std::fs::remove_dir_all(directory).unwrap();
        }
    }
    fn fixture() -> Fixture {
        static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!("rvn-image-confirmation-{}-{}-{serial}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let manager = rvn_core::persistent::PersistentDataManager::new(&directory).unwrap();
        std::fs::create_dir(directory.join("assets")).unwrap();
        let mut engine = rvn_core::Engine::new(rvn_parser::parse("\"Before\"\n\"After\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(), 16).unwrap();
        engine.step_until_interaction().unwrap(); let _ = engine.renderer.take_pending();
        let mut app = App::new();
        app.add_plugins((bevy::core::TaskPoolPlugin::default(), bevy::time::TimePlugin, bevy::asset::AssetPlugin {
                file_path: directory.join("assets").display().to_string(),
                processed_file_path: directory.join("processed-assets").display().to_string(),
                watch_for_changes_override: Some(false), ..default()
            }))
            .init_asset::<Image>().init_asset::<Font>().init_asset::<AudioSource>()
            .insert_resource(VnEngine(engine)).insert_resource(State::new(VnState::Waiting))
            .init_resource::<NextState<VnState>>().init_resource::<SaveConfirmation>()
            .init_resource::<SaveMenuState>().init_resource::<SettingsMenuState>()
            .init_resource::<crate::systems::settings_menu::Settings>()
            .init_resource::<MenuState>().init_resource::<GalleryState>().init_resource::<DialogueHistory>()
            .init_resource::<ImagemapState>().init_resource::<VnRenderState>().init_resource::<TypewriterState>()
            .init_resource::<ChoiceFocus>().init_resource::<SkipMode>().init_resource::<ScriptErrorMessage>()
            .init_resource::<CharacterRegistry>().init_resource::<CgAssetRegistry>()
            .init_resource::<crate::save_thumbnails::SaveThumbnails>()
            .init_resource::<bevy::render::view::screenshot::ScreenshotManager>()
            .init_resource::<crate::accessibility::Accessibility>().init_resource::<Theme>()
            .init_resource::<Menus>().init_resource::<crate::programmable_ui::Screens>()
            .init_resource::<bevy::ui::UiStack>().init_resource::<bevy::ui::UiScale>()
            .init_resource::<bevy::input::touch::Touches>()
            .init_resource::<ButtonInput<KeyCode>>().init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<GamepadButton>>()
            .insert_resource(LocaleConfig { available_langs: vec![],
                last_modified_current: std::time::UNIX_EPOCH, last_modified_default: std::time::UNIX_EPOCH })
            .insert_resource(ProjectPaths::new(directory.clone(), directory.clone(), directory.clone(), directory.clone(), directory.clone()))
            .insert_resource(PersistentDataResource { manager, data: rvn_core::persistent::PersistentData::default() })
            .add_event::<PlayerInput>().add_event::<VnCommand>()
            .add_event::<bevy::input::mouse::MouseButtonInput>()
            .add_event::<bevy::input::mouse::MouseWheel>()
            .add_event::<bevy::input::keyboard::KeyboardInput>()
            .add_event::<bevy::input::keyboard::KeyboardFocusLost>()
            .add_event::<bevy::window::CursorMoved>().add_event::<bevy::window::Ime>()
            .add_event::<bevy::window::WindowFocused>()
            .add_systems(PreUpdate, (
                bevy::input::mouse::mouse_button_input_system,
                bevy::input::keyboard::keyboard_input_system,
            ).chain().in_set(bevy::input::InputSystem))
            .add_systems(PreUpdate, bevy::ui::ui_focus_system
                .after(bevy::input::InputSystem).in_set(bevy::ui::UiSystem::Focus))
            .add_systems(PreUpdate, crate::editor_preview_input::project_confirmation
                .after(bevy::ui::UiSystem::Focus))
            .add_plugins(crate::source_menus::SourceMenusPlugin)
            .add_plugins(crate::menu_documents::MenuDocumentsPlugin)
            .add_systems(Update, crate::systems::player_input_system.after(update));
        app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1280.0, 720.0)
            .with_scale_factor_override(1.0), ..default() }, bevy::window::PrimaryWindow));
        use bevy::render::{render_asset::RenderAssetUsages,
            render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages}};
        let mut image = Image::new_fill(Extent3d { width:1280, height:720, depth_or_array_layers:1 },
            TextureDimension::D2, &[0,0,0,255], TextureFormat::Bgra8UnormSrgb, RenderAssetUsages::default());
        image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC | TextureUsages::COPY_DST;
        let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        app.world_mut().spawn((Camera2dBundle { camera: Camera {
            target: bevy::render::camera::RenderTarget::Image(image), ..default()
        }, ..default() }, bevy::ui::IsDefaultUiCamera));
        app.finish(); app.cleanup(); app.update();
        Fixture { app, directory, capture_sequence:0, input_sequence:0 }
    }
    fn geometry(world: &mut World, entity: Entity, size: Vec2, center: Vec2) {
        let mut node = world.get_mut::<Node>(entity).unwrap();
        let bevy::reflect::ReflectMut::Struct(fields) = node.reflect_mut() else { panic!("Node reflection") };
        fields.field_mut("calculated_size").unwrap().apply(&size);
        drop(node);
        let mut visible = ViewVisibility::default(); visible.set();
        world.entity_mut(entity).insert((GlobalTransform::from_translation(center.extend(0.0)), visible));
    }
    fn open_quickload(f: &mut Fixture) -> (Entity, Entity) {
        f.app.world_mut().send_event(PlayerInput::QuickLoad); f.app.update();
        assert!(f.app.world().resource::<SaveConfirmation>().active());
        f.app.update();
        let world = f.app.world_mut();
        let root = world.query_filtered::<Entity, With<ConfirmationRoot>>().single(world);
        let buttons: Vec<_> = world.query::<(Entity, &ConfirmationButton)>().iter(world)
            .map(|(entity, button)| (entity, button.0)).collect();
        let yes = buttons.iter().find(|(_, approve)| *approve).unwrap().0;
        let no = buttons.iter().find(|(_, approve)| !*approve).unwrap().0;
        geometry(world, root, Vec2::new(1280.0, 720.0), Vec2::new(640.0, 360.0));
        geometry(world, yes, Vec2::new(100.0, 40.0), Vec2::new(100.0, 100.0));
        geometry(world, no, Vec2::new(100.0, 40.0), Vec2::new(300.0, 100.0));
        world.resource_mut::<bevy::ui::UiStack>().uinodes = vec![root, yes, no];
        (yes, no)
    }
    fn capture(f: &mut Fixture) -> std::sync::Arc<crate::editor_preview_input::PhysicalFrame> {
        crate::editor_preview_input::begin(f.app.world_mut());
        let engine = &f.app.world().resource::<VnEngine>().0;
        f.capture_sequence = f.capture_sequence.checked_add(1).unwrap();
        let sequence = f.capture_sequence;
        let scope = FrameScope { instance: engine.instance_id(), epoch: engine.runtime_epoch(), sequence,
            control_sequence: 0, control_ack: 0, control_error: None, paused: false,
            publication_pending: false, finished: None, step_allowed: false, interaction: engine.preview_input_source(),
            position: rvn_core::scene::ResumePosition { pc: engine.state.pc, origin: None }, active: None, parked: vec![] };
        crate::editor_preview_input::capture(f.app.world(), scope).unwrap()
    }
    fn click(f: &mut Fixture, frame: std::sync::Arc<crate::editor_preview_input::PhysicalFrame>, x: f32) {
        f.input_sequence = f.input_sequence.checked_add(1).unwrap();
        let sequence = f.input_sequence;
        // This existing helper exercises completed-input cache and envelope
        // admission. Its explicit CPU pixel fixture is not GPU readback proof.
        crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![(sequence, ImageInput::Pointer {
            kind: PointerKind::Down, button: 0, point: [x, 100.0], wheel: [0.0; 2],
        })]);
        f.app.update();
    }
    fn saved_and_advanced(f: &mut Fixture) -> (usize, Vec<u8>) {
        f.app.world_mut().send_event(PlayerInput::QuickSave); f.app.update();
        let saved_pc = f.app.world().resource::<VnEngine>().0.state.pc;
        let bytes = std::fs::read(f.directory.join("quicksave.json")).unwrap();
        let mut engine = f.app.world_mut().resource_mut::<VnEngine>();
        engine.0.advance_dialogue().unwrap(); engine.0.step_until_interaction().unwrap();
        let _ = engine.0.renderer.take_pending();
        assert_ne!(engine.0.state.pc, saved_pc);
        (saved_pc, bytes)
    }
    #[test]
    fn completed_image_keyboard_control_reaches_real_confirmation_receiver_before_pointer_repair() {
        let mut f = fixture(); let (saved_pc, bytes) = saved_and_advanced(&mut f);
        open_quickload(&mut f); let frame = capture(&mut f);
        crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![
            (1, ImageInput::Key { key:"Tab".into(), logical:None, pressed:true, modifiers:[false;4] }),
        ]);
        f.app.update();
        let frame = capture(&mut f);
        crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![
            (2, ImageInput::Key { key:"Tab".into(), logical:None, pressed:false, modifiers:[false;4] }),
            (3, ImageInput::Key { key:"ReturnKey".into(), logical:None, pressed:true, modifiers:[false;4] }),
        ]);
        f.app.update();
        assert!(!f.app.world().resource::<SaveConfirmation>().active());
        assert_eq!(f.app.world().resource::<VnEngine>().0.state.pc, saved_pc);
        assert_eq!(f.app.world().resource::<VnEngine>().0.displayed_interaction().unwrap(),
            Some(rvn_core::Interaction::Dialogue { character:None, text:"Before".into() }));
        assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
    }
    #[test]
    fn completed_image_confirmation_pointer_reaches_real_receiver_and_restores_quickslot_exactly() {
        let mut f = fixture(); let (saved_pc, bytes) = saved_and_advanced(&mut f);
        open_quickload(&mut f); let frame = capture(&mut f); click(&mut f, frame, 100.0);
        assert!(!f.app.world().resource::<SaveConfirmation>().active(), "image click must reach the real confirmation receiver");
        assert_eq!(f.app.world().resource::<VnEngine>().0.state.pc, saved_pc);
        assert_eq!(f.app.world().resource::<VnEngine>().0.displayed_interaction().unwrap(),
            Some(rvn_core::Interaction::Dialogue { character: None, text: "Before".into() }));
        assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
        assert!(f.app.world().resource::<SaveConfirmation>().approved.is_none());
        assert!(f.app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn completed_image_cancel_pointer_uses_real_decision_without_loading_or_changing_quickslot() {
        let mut f = fixture(); let (_, bytes) = saved_and_advanced(&mut f);
        open_quickload(&mut f);
        let before = serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
        let history = format!("{:?}", f.app.world().resource::<VnEngine>().0.history.entries());
        let frame = capture(&mut f); click(&mut f, frame, 300.0);
        assert!(!f.app.world().resource::<SaveConfirmation>().active(), "image cancel must reach the real confirmation receiver");
        assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(), before);
        assert_eq!(format!("{:?}", f.app.world().resource::<VnEngine>().0.history.entries()), history);
        assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
        assert!(f.app.world().resource::<SaveConfirmation>().approved.is_none());
    }
    #[test]
    fn later_focus_loss_in_actual_image_batch_revokes_confirmation_press_without_a_delayed_load() {
        let mut f = fixture(); let (_, bytes) = saved_and_advanced(&mut f);
        open_quickload(&mut f);
        let before = serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
        let frame = capture(&mut f);
        crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![
            (1, ImageInput::Pointer { kind: PointerKind::Down, button: 0, point:[100.0,100.0], wheel:[0.0;2] }),
            (2, ImageInput::FocusLost),
        ]);
        f.app.update(); f.app.update();
        assert!(f.app.world().resource::<SaveConfirmation>().active());
        assert!(f.app.world().resource::<SaveConfirmation>().approved.is_none());
        assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(), before);
        assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
        assert!(!f.app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
    }

    #[test]
    fn closed_and_reopened_confirmation_cannot_inherit_an_old_image_press() {
        let mut f = fixture(); let (_, bytes) = saved_and_advanced(&mut f);
        let (old_yes, _) = open_quickload(&mut f);
        let old_frame = capture(&mut f);
        crate::editor_preview::drain_images_for_test(f.app.world_mut(), old_frame.clone(), vec![
            (1, ImageInput::Key { key:"Escape".into(), logical:None, pressed:true, modifiers:[false;4] }),
        ]);
        f.input_sequence = 1;
        f.app.update();
        assert!(!f.app.world().resource::<SaveConfirmation>().active());
        assert!(f.app.world().get_entity(old_yes).is_none());
        let (new_yes, _) = open_quickload(&mut f);
        assert_ne!(new_yes, old_yes);
        let before = serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap();
        click(&mut f, old_frame, 100.0);
        assert!(f.app.world().resource::<SaveConfirmation>().active());
        assert!(f.app.world().resource::<SaveConfirmation>().approved.is_none());
        assert_eq!(serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap(), before);
        assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
    }

    #[test]
    fn presentation_disabled_after_capture_refuses_the_original_confirm_pixels() {
        let mut f = fixture(); let (_, bytes) = saved_and_advanced(&mut f);
        let (yes, _) = open_quickload(&mut f);
        let id = f.app.world().get::<ConfirmationTarget>(yes).unwrap().0.clone();
        let page = Document::defaults().pages.into_iter()
            .find(|page| page.role == Some(rvn_ui::PageRole::Confirm)).unwrap().id;
        let frame = capture(&mut f);
        f.app.world_mut().resource_mut::<Menus>().session
            .apply_presentation(&page, &rvn_ui::Effect::Enabled(id, false));
        let before = serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap();
        click(&mut f, frame, 100.0);
        assert!(f.app.world().resource::<SaveConfirmation>().active());
        assert!(f.app.world().resource::<SaveConfirmation>().approved.is_none());
        assert_eq!(serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap(), before);
        assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
    }

    #[test]
    fn inherited_visibility_hidden_before_propagation_refuses_a_captured_confirm() {
        let mut f = fixture(); let (_, bytes) = saved_and_advanced(&mut f);
        open_quickload(&mut f);
        let frame = capture(&mut f);
        let world = f.app.world_mut();
        let root = world.query_filtered::<Entity, With<ConfirmationRoot>>().single(world);
        world.entity_mut(root).insert(Visibility::Hidden);
        let before = serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap();
        click(&mut f, frame, 100.0);
        assert!(f.app.world().resource::<SaveConfirmation>().active());
        assert!(f.app.world().resource::<SaveConfirmation>().approved.is_none());
        assert_eq!(serde_json::to_value(&f.app.world().resource::<VnEngine>().0.state).unwrap(), before);
        assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
    }

// Append inside completed_image_legacy_confirmation_contracts, reusing its
// actual image camera, plugins, envelope admission and native menu receivers.
// Private preparation only. CPU geometry does not qualify native pixels.
fn pause_image_fixture_0026() -> (Fixture, Entity, Entity) {
    use bevy::ecs::system::RunSystemOnce;
    let mut f = fixture();
    f.app.world_mut().insert_resource(State::new(VnState::Menu));
    f.app.world_mut().resource_mut::<MenuState>().return_to = Some(VnState::Waiting);
    f.app.add_event::<AppExit>().add_systems(Update,
        crate::systems::menu_interaction_system.after(crate::menu_documents::keyboard));
    f.app.add_systems(Update, (crate::systems::input_system, crate::editor_preview_input::stamp)
        .chain().run_if(in_state(VnState::Waiting))
        .after(crate::systems::menu_interaction_system).before(crate::systems::player_input_system));
    f.app.world_mut().run_system_once(crate::systems::spawn_menu_overlay);
    f.app.update(); f.app.update();
    let world = f.app.world_mut();
    let root = world.query_filtered::<Entity, With<MenuOverlay>>().single(world);
    let buttons: Vec<_> = world.query::<(Entity, &MenuButton)>().iter(world)
        .map(|(entity, button)| (entity, button.clone())).collect();
    let resume = buttons.iter().find(|(_, button)| *button == MenuButton::Resume).unwrap().0;
    geometry(world, root, Vec2::new(1280.0, 720.0), Vec2::new(640.0, 360.0));
    let mut stack = vec![root];
    for (index, (entity, _)) in buttons.iter().enumerate() {
        let center = if *entity == resume { Vec2::new(100.0, 100.0) }
            else { Vec2::new(400.0, 200.0 + index as f32 * 60.0) };
        geometry(world, *entity, Vec2::new(100.0, 40.0), center);
        stack.push(*entity);
    }
    world.resource_mut::<bevy::ui::UiStack>().uinodes = stack;
    assert!(world.get::<crate::menu_documents::legacy_navigation::Focusable>(resume).is_some());
    (f, root, resume)
}
fn pause_image_state_0026(f: &Fixture) -> Vec<u8> {
    serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap()
}
#[test]
fn pause_image_keyboard_0026_reaches_existing_resume_without_story_mutation() {
    let (mut f, _, _) = pause_image_fixture_0026(); let before = pause_image_state_0026(&f);
    let frame = capture(&mut f);
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![
        (1, ImageInput::Key { key:"Tab".into(), logical:None, pressed:true, modifiers:[false;4] })]);
    f.app.update(); let frame = capture(&mut f);
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![
        (2, ImageInput::Key { key:"Tab".into(), logical:None, pressed:false, modifiers:[false;4] }),
        (3, ImageInput::Key { key:"ReturnKey".into(), logical:None, pressed:true, modifiers:[false;4] })]);
    f.app.update();
    assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Pending(VnState::Waiting)));
    assert_eq!(f.app.world().resource::<MenuState>().return_to, None);
    assert_eq!(pause_image_state_0026(&f), before);
}
#[test]
fn pause_image_pointer_0026_reaches_existing_resume_without_story_mutation() {
    let (mut f, _, _) = pause_image_fixture_0026(); let before = pause_image_state_0026(&f);
    let frame = capture(&mut f); click(&mut f, frame, 100.0);
    assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Pending(VnState::Waiting)),
        "completed pause image must activate its actual Resume receiver");
    assert_eq!(f.app.world().resource::<MenuState>().return_to, None);
    assert_eq!(pause_image_state_0026(&f), before);
}
#[test]
fn pause_image_pointer_0026_release_after_actual_presenter_removal_cannot_advance() {
    use bevy::ecs::system::RunSystemOnce;
    let (mut f, _, _) = pause_image_fixture_0026(); let before = pause_image_state_0026(&f);
    let frame = capture(&mut f); click(&mut f, frame.clone(), 100.0);
    assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Pending(VnState::Waiting)),
        "real Resume must request the transition before testing its trailing release");
    // The CPU fixture runs no StatesPlugin. Apply the requested presenter
    // transition explicitly, using its real despawner; never alter Core state.
    f.app.world_mut().insert_resource(State::new(VnState::Waiting));
    f.app.world_mut().insert_resource(NextState::<VnState>::Unchanged);
    f.app.world_mut().run_system_once(crate::systems::despawn_menu_overlay);
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![(2,
        ImageInput::Pointer {kind:PointerKind::Up, button:0, point:[100.0,100.0], wheel:[0.0;2]})]);
    f.app.update(); f.app.update();
    assert_eq!(pause_image_state_0026(&f), before);
    assert!(!f.app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
    assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Unchanged));
}
#[test]
fn pause_image_pointer_0026_later_focus_loss_revokes_the_press() {
    let (mut f, _, _) = pause_image_fixture_0026(); let before = pause_image_state_0026(&f);
    let frame = capture(&mut f);
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![
        (1, ImageInput::Pointer {kind:PointerKind::Down, button:0, point:[100.0,100.0], wheel:[0.0;2]}),
        (2, ImageInput::FocusLost)]);
    f.app.update(); f.app.update();
    assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Unchanged));
    assert_eq!(pause_image_state_0026(&f), before);
    assert_eq!(f.app.world().resource::<MenuState>().return_to, Some(VnState::Waiting));
}
#[test]
fn pause_image_pointer_0026_new_cover_refuses_old_pause_pixels() {
    for settings in [false, true] {
        let (mut f, _, _) = pause_image_fixture_0026(); let before = pause_image_state_0026(&f);
        let frame = capture(&mut f);
        if settings {f.app.world_mut().resource_mut::<SettingsMenuState>().active = true;}
        else {f.app.world_mut().resource_mut::<SaveMenuState>().open(SaveMenuMode::Load,
            crate::systems::save_menu::SaveMenuOrigin::InGame);}
        click(&mut f, frame, 100.0);
        assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Unchanged));
        assert_eq!(pause_image_state_0026(&f), before);
        assert_eq!(f.app.world().resource::<MenuState>().return_to, Some(VnState::Waiting));
    }
}
#[test]
fn pause_image_pointer_0026_hidden_ancestor_refuses_before_visibility_propagation() {
    let (mut f, root, _) = pause_image_fixture_0026(); let before = pause_image_state_0026(&f);
    let frame = capture(&mut f); f.app.world_mut().entity_mut(root).insert(Visibility::Hidden);
    click(&mut f, frame, 100.0);
    assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Unchanged));
    assert_eq!(pause_image_state_0026(&f), before);
    assert_eq!(f.app.world().resource::<MenuState>().return_to, Some(VnState::Waiting));
}
#[test]
fn pause_image_pointer_0026_changed_intent_or_return_target_revokes_the_image() {
    for changed_action in [false, true] {
        let (mut f, _, resume) = pause_image_fixture_0026(); let before = pause_image_state_0026(&f);
        let frame = capture(&mut f);
        if changed_action {f.app.world_mut().entity_mut(resume).insert(MenuButton::Settings);}
        else {f.app.world_mut().resource_mut::<MenuState>().return_to = Some(VnState::TitleScreen);}
        let target = f.app.world().resource::<MenuState>().return_to.clone();
        click(&mut f, frame, 100.0);
        assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Unchanged));
        assert!(!f.app.world().resource::<SettingsMenuState>().active);
        assert_eq!(pause_image_state_0026(&f), before);
        assert_eq!(f.app.world().resource::<MenuState>().return_to, target);
    }
}
#[test]
fn pause_image_pointer_0026_short_down_up_batch_keeps_the_actual_press_recipient() {
    let (mut f, _, _) = pause_image_fixture_0026(); let before = pause_image_state_0026(&f);
    let frame = capture(&mut f);
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![
        (1, ImageInput::Pointer {kind:PointerKind::Down, button:0, point:[100.0,100.0], wheel:[0.0;2]}),
        (2, ImageInput::Pointer {kind:PointerKind::Up, button:0, point:[100.0,100.0], wheel:[0.0;2]})]);
    f.app.update();
    assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Pending(VnState::Waiting)),
        "short OS clicks must retain their actual primary Down recipient");
    assert_eq!(pause_image_state_0026(&f), before);
}
#[test]
fn pause_image_pointer_0026_drag_from_background_does_not_press_resume() {
    let (mut f, _, _) = pause_image_fixture_0026(); let before = pause_image_state_0026(&f);
    let frame = capture(&mut f);
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame.clone(), vec![
        (1, ImageInput::Pointer {kind:PointerKind::Down, button:0, point:[700.0,100.0], wheel:[0.0;2]})]);
    f.app.update();
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![
        (2, ImageInput::Pointer {kind:PointerKind::Move, button:0, point:[100.0,100.0], wheel:[0.0;2]})]);
    f.app.update();
    assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Unchanged));
    assert_eq!(pause_image_state_0026(&f), before);
    assert_eq!(f.app.world().resource::<MenuState>().return_to, Some(VnState::Waiting));
}
#[test]
fn pause_image_pointer_0026_secondary_button_cannot_activate_resume() {
    let (mut f, _, _) = pause_image_fixture_0026(); let before = pause_image_state_0026(&f);
    let frame = capture(&mut f);
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![
        (1, ImageInput::Pointer {kind:PointerKind::Down, button:1, point:[100.0,100.0], wheel:[0.0;2]})]);
    f.app.update();
    assert!(matches!(f.app.world().resource::<NextState<VnState>>(), NextState::Unchanged));
    assert_eq!(pause_image_state_0026(&f), before);
}


// Uses the real confirmation receiver, SaveManager, epoch change and admission.
// Explicit CPU geometry; this is not evidence of GPU/native parity.
fn stale_image_scene_fixture_0028() -> Fixture {
    let mut f = fixture();
    let mut engine = rvn_core::Engine::from_program(rvn_parser::parse_program(
        "init{set marker=1}\nscene label start\n\"Before\"\n\"After\"\n").unwrap(),
        crate::bevy_renderer::BevyRenderer::new(), 16).unwrap();
    engine.enter_label("start").unwrap(); engine.step_until_interaction().unwrap();
    f.app.insert_resource(VnEngine(engine)).init_resource::<crate::scene_publication::ScenePresentation>()
        .add_systems(First, crate::scene_publication::consume);
    crate::scene_publication::consume(f.app.world_mut());
    assert!(!f.app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
    assert!(!f.app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
    assert!(f.app.world().resource::<VnEngine>().0.state.scene_execution.is_some());
    f
}
fn stale_image_loaded_fixture_0028() -> (Fixture, std::sync::Arc<crate::editor_preview_input::PhysicalFrame>, Vec<u8>) {
    let mut f = stale_image_scene_fixture_0028();
    let (saved_pc, bytes) = saved_and_advanced(&mut f);
    crate::scene_publication::consume(f.app.world_mut());
    open_quickload(&mut f);
    let frame = capture(&mut f);
    let old_epoch = frame.scope.epoch;
    click(&mut f, frame.clone(), 100.0);
    assert_eq!(f.app.world().resource::<VnEngine>().0.state.pc, saved_pc);
    assert_ne!(f.app.world().resource::<VnEngine>().0.runtime_epoch(), old_epoch);
    assert!(!f.app.world().resource::<SaveConfirmation>().active());
    // Real next input-frame preparation cancels the old physical generation.
    crate::editor_preview_input::begin(f.app.world_mut());
    (f, frame, bytes)
}
#[test]
fn stale_image_0028_release_after_real_quickload_preserves_state_and_success_status() {
    let (mut f, frame, bytes) = stale_image_loaded_fixture_0028();
    let before = serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![(2,
        ImageInput::Pointer {kind:PointerKind::Up, button:0, point:[100.0,100.0], wheel:[0.0;2]})]);
    f.app.update(); f.app.update();
    assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(), before);
    assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
    assert!(!f.app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
    let (received, accepted, error) = crate::editor_preview::image_input_status_for_test(f.app.world());
    assert_eq!(received, 2);
    assert!(error.is_none(), "a retired release must not report a failed restore: {error:?}");
    assert_eq!(accepted, 2, "retirement must be received by the backend, not a fabricated frontend ACK");
}
#[test]
fn stale_image_0028_focus_loss_after_real_quickload_preserves_state_and_success_status() {
    let (mut f, frame, bytes) = stale_image_loaded_fixture_0028();
    let before = serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![(2, ImageInput::FocusLost)]);
    f.app.update();
    assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(), before);
    assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
    let (received, accepted, error) = crate::editor_preview::image_input_status_for_test(f.app.world());
    assert_eq!(received, 2);
    assert!(error.is_none(), "retired focus loss must not report a failed restore: {error:?}");
    assert_eq!(accepted, 2);
}
#[test]
fn stale_image_0028_old_positive_press_still_refuses_the_previous_session() {
    let (mut f, frame, bytes) = stale_image_loaded_fixture_0028();
    let before = serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), frame, vec![(2,
        ImageInput::Pointer {kind:PointerKind::Down, button:0, point:[100.0,100.0], wheel:[0.0;2]})]);
    f.app.update();
    assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(), before);
    assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
    let (received, accepted, error) = crate::editor_preview::image_input_status_for_test(f.app.world());
    assert_eq!(received, 2);
    assert_eq!(accepted, 1);
    assert_eq!(error.as_deref(), Some("The rendered engine session changed"));
}


// Additional regression inside the real confirmation fixture after retirement
// transport adoption. Private only, not installed/typechecked/executed.
#[test]
fn stale_image_0028_retired_release_and_focus_loss_cannot_clear_a_new_pointer_capture() {
    let (mut f, old, bytes) = stale_image_loaded_fixture_0028();
    // A fresh positive press needs the actual restored presentation installed.
    crate::scene_publication::consume(f.app.world_mut());
    assert!(!f.app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
    assert!(!f.app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
    let current = capture(&mut f);
    let before = serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), current.clone(), vec![(2,
        ImageInput::Pointer {kind:PointerKind::Down, button:0, point:[640.0,360.0], wheel:[0.0;2]})]);
    assert_eq!(crate::editor_preview::image_input_status_for_test(f.app.world()), (2,2,None));
    // Consume the actual fresh MouseButtonInput with Bevy's physical receiver;
    // Use the same persistent schedule/readers, not new RunSystemOnce locals.
    // Do not advance Update or consume the intentional new story action here.
    f.app.world_mut().run_schedule(PreUpdate);
    assert!(f.app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), old, vec![
        (3, ImageInput::Pointer {kind:PointerKind::Up, button:0, point:[100.0,100.0], wheel:[0.0;2]}),
        (4, ImageInput::FocusLost)]);
    assert!(f.app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left),
        "obsolete cancellation must not neutralize the new generation's press");
    assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(), before);
    assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
    assert_eq!(crate::editor_preview::image_input_status_for_test(f.app.world()), (4,4,None));
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), current, vec![(5,
        ImageInput::Pointer {kind:PointerKind::Up, button:0, point:[640.0,360.0], wheel:[0.0;2]})]);
    f.app.world_mut().run_schedule(PreUpdate);
    assert!(!f.app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left),
        "the current Up must still reach the actual physical receiver");
}
#[test]
fn stale_image_0028_key_release_after_real_quickload_is_a_retirement_without_story_mutation() {
    let mut f = stale_image_scene_fixture_0028(); let (saved_pc, bytes) = saved_and_advanced(&mut f);
    crate::scene_publication::consume(f.app.world_mut());
    open_quickload(&mut f); let old = capture(&mut f);
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), old.clone(), vec![(1,
        ImageInput::Key {key:"KeyA".into(),logical:Some("a".into()),pressed:true,modifiers:[false;4]})]);
    f.input_sequence=1; f.app.update();
    assert!(f.app.world().resource::<SaveConfirmation>().active());
    click(&mut f, old.clone(), 100.0);
    assert_eq!(f.app.world().resource::<VnEngine>().0.state.pc, saved_pc);
    assert_ne!(old.scope.epoch, f.app.world().resource::<VnEngine>().0.runtime_epoch());
    crate::editor_preview_input::begin(f.app.world_mut());
    let before = serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
    crate::editor_preview::drain_images_for_test(f.app.world_mut(), old, vec![(3,
        ImageInput::Key {key:"KeyA".into(),logical:Some("a".into()),pressed:false,modifiers:[false;4]})]);
    f.app.update();
    assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(), before);
    assert_eq!(std::fs::read(f.directory.join("quicksave.json")).unwrap(), bytes);
    assert_eq!(crate::editor_preview::image_input_status_for_test(f.app.world()), (3,3,None));
    assert!(!f.app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::KeyA));
}

}
