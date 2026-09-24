use bevy::prelude::*;
use rvn_core::save::SaveManager;

use crate::project_paths::ProjectPaths;
#[cfg(target_arch = "wasm32")]
use crate::resources::PendingMusicPlayback;
use crate::resources::{
    DialogueHistory, ImagemapState, MenuState, MusicAssetRegistry, MusicEntity, MusicPlaybackState,
    MusicVolume, PersistentDataResource, ProjectTitle, Theme, TitleAnchor, TitleBackgroundMode,
    TitleButtonAlign, TitleButtonStyle, TitleScreenTheme, TypewriterState, VnEngine, VnRenderState,
    VnState,
};
use crate::systems::audio::spawn_music;
use crate::systems::save_menu::{
    apply_loaded_game, record_resume_target, SaveMenuMode, SaveMenuOrigin, SaveMenuState,
};
use crate::systems::settings_menu::SettingsMenuState;
use crate::vn_command::VnCommand;
use rvn_core::persistent::{LastResumeTarget, ResumeSaveKind};
use rvn_core::save::{SaveData, SaveError};

// ─── Composants locaux ───────────────────────────────────────────────────────

#[derive(Component)]
pub struct TitleOverlay;

#[derive(Component)]
pub struct TitleBackgroundSprite {
    mode: TitleBackgroundMode,
}

#[derive(Component, Clone, Copy)]
pub struct TitleButtonColors {
    pub(crate) normal: Color,
    pub(crate) hover: Color,
    pub(crate) pressed: Color,
}

#[derive(Component, Clone, PartialEq)]
pub enum TitleButton {
    Continue,
    NewGame,
    LoadGame,
    Gallery,
    Settings,
    Quit,
}

// ─── Spawn / Despawn ─────────────────────────────────────────────────────────

pub fn spawn_title_screen(
    mut commands: Commands,
    theme: Res<Theme>,
    asset_server: Res<AssetServer>,
    project_paths: Res<ProjectPaths>,
    project_title: Res<ProjectTitle>,
    persistent: Res<PersistentDataResource>,
    mut music_entity: ResMut<MusicEntity>,
    mut music_playback: ResMut<MusicPlaybackState>,
    music_registry: Res<MusicAssetRegistry>,
    music_volume: Res<MusicVolume>,
    mut next_state: ResMut<NextState<VnState>>,
) {
    let title_theme = &theme.title_screen;
    if !title_theme.enabled {
        next_state.set(VnState::Stepping);
        return;
    }
    let title_font: Handle<Font> = theme
        .text
        .name
        .font_path
        .as_ref()
        .map(|p| asset_server.load(p.clone()))
        .unwrap_or_default();
    let title_text = title_theme
        .title
        .text
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| {
            if project_title.0.trim().is_empty() {
                "MON VISUAL NOVEL"
            } else {
                project_title.0.as_str()
            }
        });

    play_title_music(
        &mut commands,
        &asset_server,
        &project_paths,
        &mut music_entity,
        &mut music_playback,
        &music_registry,
        &music_volume,
        title_theme.music.as_deref(),
    );

    if let Some(background) = title_theme.background.as_ref() {
        if let Some(path) = existing_asset_path(&project_paths, background.path()) {
            commands.spawn((
                TitleOverlay,
                TitleBackgroundSprite {
                    mode: background.mode(),
                },
                SpriteBundle {
                    texture: asset_server.load(path),
                    transform: Transform::from_xyz(0.0, 0.0, -200.0),
                    ..default()
                },
            ));
        } else if let Some(path) = background.path() {
            warn!(
                "[titre] fond introuvable: {}",
                project_paths.assets.join(path).display()
            );
        }
    }

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                background_color: Color::srgba(0.05, 0.05, 0.1, 1.0).into(),
                ..default()
            },
            TitleOverlay,
        ))
        .with_children(|parent| {
            if let Some(logo_config) = title_theme.logo.as_ref() {
                if let Some(logo_path) = logo_config.path() {
                    if existing_asset_path(&project_paths, Some(logo_path)).is_some() {
                        let logo_size =
                            Vec2::new(420.0 * logo_config.scale(), 160.0 * logo_config.scale());
                        parent.spawn(ImageBundle {
                            style: anchored_style(
                                logo_config.anchor(),
                                logo_config.offset_x(),
                                logo_config.offset_y(),
                                Some(logo_size),
                            ),
                            image: UiImage::new(asset_server.load(logo_path.to_string())),
                            z_index: ZIndex::Global(1),
                            ..default()
                        });
                    } else {
                        warn!(
                            "[titre] logo introuvable: {}",
                            project_paths.assets.join(logo_path).display()
                        );
                    }
                }
            }

            parent.spawn(
                TextBundle::from_section(
                    title_text,
                    TextStyle {
                        font: title_font.clone(),
                        font_size: title_theme.title.font_size,
                        color: color_from_hex(title_theme.title.color.as_deref(), Color::WHITE),
                    },
                )
                .with_text_justify(title_justify(title_theme))
                .with_style(Style {
                    width: Val::Percent(100.0),
                    ..title_style(title_theme)
                }),
            );

            let buttons_anchor = title_theme
                .buttons
                .anchor
                .clone()
                .unwrap_or(TitleAnchor::Center);
            let buttons_offset_x = title_theme.buttons.offset_x.unwrap_or_else(|| {
                ((title_theme.button_x - anchor_default_x(&buttons_anchor)) * 1280.0).round()
            });
            let buttons_offset_y = title_theme.buttons.offset_y.unwrap_or_else(|| {
                ((title_theme.button_y - anchor_default_y(&buttons_anchor)) * 720.0).round()
            });
            let button_spacing = title_theme
                .buttons
                .spacing
                .unwrap_or(title_theme.button_spacing);

            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(title_theme.buttons.width),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(button_spacing),
                        align_items: title_align_items(title_theme.button_align.clone()),
                        ..anchored_style(
                            buttons_anchor,
                            buttons_offset_x,
                            buttons_offset_y,
                            Some(Vec2::new(title_theme.buttons.width, 0.0)),
                        )
                    },
                    background_color: Color::NONE.into(),
                    z_index: ZIndex::Global(10),
                    ..default()
                })
                .with_children(|buttons| {
                    let labels = &title_theme.buttons.labels;
                    let visibility = &title_theme.buttons.visibility;
                    let continue_available = title_continue_available(&project_paths, &persistent);
                    for button_id in &title_theme.button_order {
                        match button_id.as_str() {
                            "continue"
                                if visibility
                                    .continue_button
                                    .unwrap_or(title_theme.show_continue)
                                    && continue_available =>
                            {
                                spawn_configured_title_button(
                                    buttons,
                                    labels.continue_label.as_deref().unwrap_or("Continuer"),
                                    TitleButton::Continue,
                                    title_font.clone(),
                                    title_theme,
                                );
                            }
                            "new_game"
                                if visibility.new_game.unwrap_or(title_theme.show_new_game) =>
                            {
                                spawn_configured_title_button(
                                    buttons,
                                    labels.new_game.as_deref().unwrap_or("Nouvelle Partie"),
                                    TitleButton::NewGame,
                                    title_font.clone(),
                                    title_theme,
                                );
                            }
                            "load" if visibility.load.unwrap_or(title_theme.show_load) => {
                                spawn_configured_title_button(
                                    buttons,
                                    labels.load.as_deref().unwrap_or("Charger la Partie"),
                                    TitleButton::LoadGame,
                                    title_font.clone(),
                                    title_theme,
                                );
                            }
                            "gallery" if visibility.gallery.unwrap_or(title_theme.show_gallery) => {
                                spawn_configured_title_button(
                                    buttons,
                                    labels.gallery.as_deref().unwrap_or("Galerie"),
                                    TitleButton::Gallery,
                                    title_font.clone(),
                                    title_theme,
                                );
                            }
                            "settings"
                                if visibility.settings.unwrap_or(title_theme.show_settings) =>
                            {
                                spawn_configured_title_button(
                                    buttons,
                                    labels.settings.as_deref().unwrap_or("Paramètres"),
                                    TitleButton::Settings,
                                    title_font.clone(),
                                    title_theme,
                                );
                            }
                            "quit" if visibility.quit.unwrap_or(title_theme.show_quit) => {
                                spawn_configured_title_button(
                                    buttons,
                                    labels.quit.as_deref().unwrap_or("Quitter"),
                                    TitleButton::Quit,
                                    title_font.clone(),
                                    title_theme,
                                );
                            }
                            unknown
                                if !matches!(
                                    unknown,
                                    "continue"
                                        | "new_game"
                                        | "load"
                                        | "gallery"
                                        | "settings"
                                        | "quit"
                                ) =>
                            {
                                warn!("[titre] bouton inconnu dans button_order: {unknown}");
                            }
                            _ => {}
                        }
                    }
                });
        });
}

fn existing_asset_path(project_paths: &ProjectPaths, path: Option<&str>) -> Option<String> {
    let path = path?;
    #[cfg(target_arch = "wasm32")]
    {
        let _ = project_paths;
        return Some(path.to_string());
    }

    #[cfg(not(target_arch = "wasm32"))]
    if project_paths.assets.join(path).exists() {
        Some(path.to_string())
    } else {
        None
    }
}

fn title_style(theme: &TitleScreenTheme) -> Style {
    if let Some(anchor) = theme.title.anchor.clone() {
        let offset_x = theme.title.offset_x.unwrap_or(0.0);
        let offset_y = theme.title.offset_y.unwrap_or(96.0);
        let mut style = Style {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            ..default()
        };
        match anchor {
            TitleAnchor::TopLeft | TitleAnchor::TopCenter | TitleAnchor::TopRight => {
                style.top = Val::Px(offset_y);
            }
            TitleAnchor::CenterLeft | TitleAnchor::Center | TitleAnchor::CenterRight => {
                style.top = Val::Percent(50.0);
                style.margin.top = Val::Px(offset_y);
            }
            TitleAnchor::BottomLeft | TitleAnchor::BottomCenter | TitleAnchor::BottomRight => {
                style.bottom = Val::Px(-offset_y);
            }
        }
        match anchor {
            TitleAnchor::TopLeft | TitleAnchor::CenterLeft | TitleAnchor::BottomLeft => {
                style.left = Val::Px(offset_x);
            }
            TitleAnchor::TopCenter | TitleAnchor::Center | TitleAnchor::BottomCenter => {
                style.left = Val::Px(offset_x);
            }
            TitleAnchor::TopRight | TitleAnchor::CenterRight | TitleAnchor::BottomRight => {
                style.right = Val::Px(-offset_x);
            }
        }
        return style;
    }

    Style {
        position_type: PositionType::Absolute,
        left: Val::Percent((theme.title.x * 100.0).clamp(0.0, 100.0)),
        top: Val::Percent((theme.title.y * 100.0).clamp(0.0, 100.0)),
        margin: UiRect::left(Val::Percent(-50.0)),
        ..default()
    }
}

fn title_justify(theme: &TitleScreenTheme) -> JustifyText {
    match theme.title.anchor.as_ref() {
        Some(TitleAnchor::TopLeft | TitleAnchor::CenterLeft | TitleAnchor::BottomLeft) => {
            JustifyText::Left
        }
        Some(TitleAnchor::TopRight | TitleAnchor::CenterRight | TitleAnchor::BottomRight) => {
            JustifyText::Right
        }
        _ => JustifyText::Center,
    }
}

fn anchored_style(anchor: TitleAnchor, offset_x: f32, offset_y: f32, size: Option<Vec2>) -> Style {
    let mut style = Style {
        position_type: PositionType::Absolute,
        ..default()
    };
    let width = size.map(|size| size.x.max(0.0));
    let height = size.map(|size| size.y.max(0.0));

    if let Some(width) = width {
        style.width = Val::Px(width);
    }
    if let Some(height) = height {
        style.height = Val::Px(height);
    }

    match anchor {
        TitleAnchor::TopLeft => {
            style.left = Val::Px(offset_x);
            style.top = Val::Px(offset_y);
        }
        TitleAnchor::TopCenter => {
            style.left = Val::Percent(50.0);
            style.top = Val::Px(offset_y);
            style.margin.left = Val::Px(offset_x - width.unwrap_or(0.0) / 2.0);
        }
        TitleAnchor::TopRight => {
            style.right = Val::Px(-offset_x);
            style.top = Val::Px(offset_y);
        }
        TitleAnchor::CenterLeft => {
            style.left = Val::Px(offset_x);
            style.top = Val::Percent(50.0);
            style.margin.top = Val::Px(offset_y - height.unwrap_or(0.0) / 2.0);
        }
        TitleAnchor::Center => {
            style.left = Val::Percent(50.0);
            style.top = Val::Percent(50.0);
            style.margin.left = Val::Px(offset_x - width.unwrap_or(0.0) / 2.0);
            style.margin.top = Val::Px(offset_y - height.unwrap_or(0.0) / 2.0);
        }
        TitleAnchor::CenterRight => {
            style.right = Val::Px(-offset_x);
            style.top = Val::Percent(50.0);
            style.margin.top = Val::Px(offset_y - height.unwrap_or(0.0) / 2.0);
        }
        TitleAnchor::BottomLeft => {
            style.left = Val::Px(offset_x);
            style.bottom = Val::Px(-offset_y);
        }
        TitleAnchor::BottomCenter => {
            style.left = Val::Percent(50.0);
            style.bottom = Val::Px(-offset_y);
            style.margin.left = Val::Px(offset_x - width.unwrap_or(0.0) / 2.0);
        }
        TitleAnchor::BottomRight => {
            style.right = Val::Px(-offset_x);
            style.bottom = Val::Px(-offset_y);
        }
    }
    style
}

fn anchor_default_x(anchor: &TitleAnchor) -> f32 {
    match anchor {
        TitleAnchor::TopLeft | TitleAnchor::CenterLeft | TitleAnchor::BottomLeft => 0.0,
        TitleAnchor::TopCenter | TitleAnchor::Center | TitleAnchor::BottomCenter => 0.5,
        TitleAnchor::TopRight | TitleAnchor::CenterRight | TitleAnchor::BottomRight => 1.0,
    }
}

fn anchor_default_y(anchor: &TitleAnchor) -> f32 {
    match anchor {
        TitleAnchor::TopLeft | TitleAnchor::TopCenter | TitleAnchor::TopRight => 0.0,
        TitleAnchor::CenterLeft | TitleAnchor::Center | TitleAnchor::CenterRight => 0.5,
        TitleAnchor::BottomLeft | TitleAnchor::BottomCenter | TitleAnchor::BottomRight => 1.0,
    }
}

fn color_from_hex(raw: Option<&str>, fallback: Color) -> Color {
    raw.and_then(|value| Srgba::hex(value).ok())
        .map(Color::from)
        .unwrap_or(fallback)
}

fn title_button_colors(style: &TitleButtonStyle) -> TitleButtonColors {
    TitleButtonColors {
        normal: color_from_hex(
            Some(&style.background_color),
            Color::srgba(0.15, 0.15, 0.25, 1.0),
        ),
        hover: color_from_hex(
            Some(&style.hover_color),
            Color::srgba(0.25, 0.25, 0.45, 1.0),
        ),
        pressed: color_from_hex(
            Some(&style.pressed_color),
            Color::srgba(0.35, 0.35, 0.65, 1.0),
        ),
    }
}

fn play_title_music(
    commands: &mut Commands,
    asset_server: &AssetServer,
    project_paths: &ProjectPaths,
    music_entity: &mut MusicEntity,
    _music_playback: &mut MusicPlaybackState,
    music_registry: &MusicAssetRegistry,
    music_volume: &MusicVolume,
    music: Option<&str>,
) {
    let Some(music) = music.filter(|m| !m.trim().is_empty()) else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let _ = project_paths;
    #[cfg(not(target_arch = "wasm32"))]
    if !project_paths.assets.join(music).exists() {
        warn!(
            "[titre] musique introuvable: {}",
            project_paths.assets.join(music).display()
        );
        return;
    }
    #[cfg(target_arch = "wasm32")]
    {
        _music_playback.last_request = Some(PendingMusicPlayback {
            file: music.to_string(),
            fade_ms: None,
        });
        _music_playback.web_music_replay_pending = true;
    }
    spawn_music(
        commands,
        asset_server,
        music_registry,
        music_entity,
        music_volume,
        music,
        None,
    );
    info!("[titre] musique {}", music);
}

fn title_align_items(align: TitleButtonAlign) -> AlignItems {
    match align {
        TitleButtonAlign::Left => AlignItems::FlexStart,
        TitleButtonAlign::Center => AlignItems::Center,
        TitleButtonAlign::Right => AlignItems::FlexEnd,
    }
}

fn title_continue_available(
    project_paths: &ProjectPaths,
    persistent: &PersistentDataResource,
) -> bool {
    SaveManager::new(
        &project_paths.saves,
        crate::systems::save_menu::SUPPORTED_SLOTS,
    )
    .map(|mgr| resolve_continue_data(&mgr, persistent).is_ok())
    .unwrap_or(false)
}

pub(crate) fn resolve_continue_data(
    mgr: &SaveManager,
    persistent: &PersistentDataResource,
) -> Result<(SaveData, Option<LastResumeTarget>), SaveError> {
    if let Some(target) = persistent.data.last_resume_target.as_ref() {
        match load_resume_target(mgr, target) {
            Ok(data) => return Ok((data, Some(target.clone()))),
            Err(e) => {
                warn!("[titre] point de reprise ignoré: {e}");
            }
        }
    }

    if mgr.autosave_exists() {
        let data = mgr.load_autosave()?;
        let target = LastResumeTarget::autosave(data.timestamp);
        return Ok((data, Some(target)));
    }

    if let Some(data) = mgr.latest_manual_save() {
        let target = LastResumeTarget::manual(data.slot, data.timestamp);
        return Ok((data, Some(target)));
    }

    Err(SaveError::SlotVide(0))
}

fn load_resume_target(mgr: &SaveManager, target: &LastResumeTarget) -> Result<SaveData, SaveError> {
    let data = match target.kind {
        ResumeSaveKind::Autosave => mgr.load_autosave()?,
        ResumeSaveKind::Manual => mgr.load(target.slot.unwrap_or(0))?,
        ResumeSaveKind::Quicksave => mgr.load_quicksave()?,
    };

    if data.timestamp == target.timestamp {
        Ok(data)
    } else {
        Err(SaveError::SlotVide(target.slot.unwrap_or(0)))
    }
}

fn spawn_title_button(
    parent: &mut ChildBuilder,
    label: &str,
    action: TitleButton,
    font: Handle<Font>,
    width: f32,
    height: f32,
    font_size: f32,
    colors: TitleButtonColors,
    text_color: Color,
) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(width),
                    height: Val::Px(height),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: colors.normal.into(),
                ..default()
            },
            action,
            colors,
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font,
                    font_size,
                    color: text_color,
                },
            ));
        });
}

fn spawn_configured_title_button(
    parent: &mut ChildBuilder,
    label: &str,
    action: TitleButton,
    font: Handle<Font>,
    theme: &TitleScreenTheme,
) {
    let colors = title_button_colors(&theme.buttons.style);
    let text_color = color_from_hex(Some(&theme.buttons.style.text_color), Color::WHITE);
    spawn_title_button(
        parent,
        label,
        action,
        font,
        theme.buttons.width,
        theme.buttons.height,
        theme.buttons.font_size,
        colors,
        text_color,
    );
}

pub fn despawn_title_screen(
    mut commands: Commands,
    overlay_query: Query<Entity, With<TitleOverlay>>,
) {
    for entity in overlay_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

pub fn title_background_resize_system(
    windows: Query<&Window>,
    images: Res<Assets<Image>>,
    mut query: Query<(&Handle<Image>, &TitleBackgroundSprite, &mut Transform)>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let window_w = window.resolution.width().max(1.0);
    let window_h = window.resolution.height().max(1.0);

    for (handle, bg, mut transform) in query.iter_mut() {
        let Some(image) = images.get(handle) else {
            continue;
        };
        let size = image.size();
        let image_w = (size.x as f32).max(1.0);
        let image_h = (size.y as f32).max(1.0);
        let scale = match bg.mode {
            TitleBackgroundMode::Cover => (window_w / image_w).max(window_h / image_h),
            TitleBackgroundMode::Contain => (window_w / image_w).min(window_h / image_h),
            TitleBackgroundMode::Stretch => {
                transform.scale = Vec3::new(window_w / image_w, window_h / image_h, 1.0);
                transform.translation.x = 0.0;
                transform.translation.y = 0.0;
                continue;
            }
        };
        transform.scale = Vec3::new(scale, scale, 1.0);
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
    }
}

// ─── Interaction ─────────────────────────────────────────────────────────────

pub fn title_interaction_system(
    mut interaction_query: Query<
        (
            &Interaction,
            &TitleButton,
            &TitleButtonColors,
            &mut BackgroundColor,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut next_state: ResMut<NextState<VnState>>,
    mut engine: ResMut<VnEngine>,
    mut render_state: ResMut<VnRenderState>,
    mut imagemap_state: ResMut<ImagemapState>,
    mut tw_state: ResMut<TypewriterState>,
    mut history: ResMut<DialogueHistory>,
    project_paths: Res<ProjectPaths>,
    mut menu_state: ResMut<MenuState>,
    mut save_menu_state: ResMut<SaveMenuState>,
    mut settings_menu_state: ResMut<SettingsMenuState>,
    mut persistent: ResMut<PersistentDataResource>,
    mut vn_events: EventWriter<VnCommand>,
    mut exit: EventWriter<AppExit>,
) {
    for (interaction, button, colors, mut bg_color) in interaction_query.iter_mut() {
        match interaction {
            Interaction::Hovered => {
                *bg_color = colors.hover.into();
            }
            Interaction::None => {
                *bg_color = colors.normal.into();
            }
            Interaction::Pressed => {
                *bg_color = colors.pressed.into();

                match button {
                    TitleButton::Continue => {
                        match SaveManager::new(
                            &project_paths.saves,
                            crate::systems::save_menu::SUPPORTED_SLOTS,
                        ) {
                            Ok(mgr) => match resolve_continue_data(&mgr, &persistent) {
                                Ok((data, target)) => {
                                    if let Some(target) = target {
                                        record_resume_target(&mut persistent, target);
                                    }
                                    engine.0.load_data(data);
                                    apply_loaded_game(
                                        &mut engine,
                                        &mut render_state,
                                        &mut imagemap_state,
                                        &mut tw_state,
                                        &mut history,
                                        &mut vn_events,
                                    );
                                    next_state.set(VnState::Waiting);
                                }
                                Err(e) => error!("[titre] reprise impossible: {e}"),
                            },
                            Err(e) => error!("[titre] SaveManager indisponible: {e}"),
                        }
                    }

                    TitleButton::NewGame => {
                        // Returning here after an ending must start a fresh story,
                        // not resume the finished instruction pointer.
                        if engine.0.is_finished() {
                            let Ok(mut fresh) = engine
                                .0
                                .fresh(crate::bevy_renderer::BevyRenderer::new(), 64)
                            else {
                                error!("Impossible de recommencer la partie");
                                continue;
                            };
                            fresh.locale = engine.0.locale.take();
                            fresh.persistent_vars = engine.0.persistent_vars.clone();
                            for (id, sprite) in &engine.0.state.sprites {
                                if sprite.visible {
                                    vn_events.send(VnCommand::HideSprite {
                                        id: id.clone(),
                                        transition: rvn_parser::Transition::None,
                                    });
                                }
                            }
                            engine.0 = fresh;
                            *render_state = VnRenderState::default();
                            *tw_state = TypewriterState::default();
                            imagemap_state.clear();
                            history.clear();
                        }
                        info!("[titre] nouvelle partie");
                        next_state.set(VnState::Stepping);
                    }

                    TitleButton::LoadGame => {
                        menu_state.return_to = Some(VnState::TitleScreen);
                        save_menu_state.open(SaveMenuMode::Load, SaveMenuOrigin::TitleScreen);
                        next_state.set(VnState::Menu);
                    }

                    TitleButton::Gallery => {
                        next_state.set(VnState::Gallery);
                    }

                    TitleButton::Settings => {
                        menu_state.return_to = Some(VnState::TitleScreen);
                        settings_menu_state.active = true;
                        next_state.set(VnState::Menu);
                    }

                    TitleButton::Quit => {
                        exit.send(AppExit::Success);
                    }
                }
            }
        }
    }
}
