//! Player accessibility is independent of story history. F8 opens a keyboard
//! and pointer-operated panel even when the project has a custom title menu.
#[cfg(target_arch = "wasm32")]
use crate::speech_web::Speech;
#[cfg(not(target_arch = "wasm32"))]
use crate::speech_worker::Speech;
use crate::{
    resources::PersistentDataResource, systems::settings_menu::Settings, vn_command::VnCommand,
};
use bevy::prelude::*;
use rvn_ui::accessibility::{AccessibilitySettings, SpeechRequest};

#[derive(Resource, Default)]
pub(crate) struct Accessibility {
    pub settings: AccessibilitySettings,
    pub open: bool,
    pub blocked: bool,
    authored: AccessibilitySettings,
    player: Option<AccessibilitySettings>,
    initialized: bool,
    status: String,
    focus: usize,
    last_dialogue: String,
    last_focus: Option<(String, String)>,
    last_native_focus: Option<Entity>,
    redraw: bool,
    language: String,
    dialogue_scroll: f32,
    dialogue_overflow: f32,
    legacy_notice_until: f64,
}
#[derive(Component)]
struct PanelRoot;
#[derive(Component)]
struct PanelButton(usize);
#[derive(Component)]
struct PanelText;
#[derive(Component)]
struct PanelViewport;
#[derive(Component)]
struct PanelRows;
#[derive(Component)]
struct VoiceNotice;
#[derive(Component)]
struct VoiceNoticeText;
#[derive(Component, Default)]
struct OriginalText(Vec<(f32, Color, f32, Color)>);
#[derive(Component)]
struct OriginalBackground(Color, Color);
impl Accessibility {
    #[cfg(target_arch = "wasm32")]
    pub(super) fn browser_error(&mut self, problem: String) {
        self.status = problem;
        self.redraw = true;
    }
}
pub(crate) struct AccessibilityPlugin;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AccessibilityInput;
impl Plugin for AccessibilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Accessibility>()
            .add_event::<bevy::a11y::ActionRequest>()
            .insert_non_send_resource(Speech::default())
            .add_systems(First, reset_text)
            .add_systems(
                PreUpdate,
                keyboard
                    .after(bevy::input::InputSystem)
                    .after(bevy::ui::UiSystem::Focus)
                    .in_set(AccessibilityInput),
            )
            .add_systems(
                PreUpdate,
                assistive_panel
                    .after(keyboard)
                    .after(crate::programmable_ui::InterfaceFocusSet),
            )
            .add_systems(
                Update,
                (
                    receive,
                    buttons.after(receive),
                    voice_status.after(receive),
                    panel.after(buttons).after(voice_status),
                    notice.after(voice_status),
                    focused.after(crate::programmable_ui::InterfaceInputSet),
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    visuals.before(bevy::ui::widget::measure_text_system),
                    dialogue_layout.before(bevy::ui::UiSystem::Layout),
                    panel_focus.before(bevy::ui::UiSystem::Layout),
                    panel_scroll.after(bevy::ui::UiSystem::Layout),
                    semantics
                        .after(bevy::ui::UiSystem::Layout)
                        .after(bevy::transform::TransformSystem::TransformPropagate),
                    interface_labels
                        .after(bevy::ui::UiSystem::Layout)
                        .after(bevy::transform::TransformSystem::TransformPropagate),
                ),
            );
        // New UI entities inherit visibility during PostUpdate, not at spawn.
        // Reading it earlier briefly hides every rebuilt control from assistive
        // tools and causes browser focus to disappear during binding changes.
        app.configure_sets(
            PostUpdate,
            bevy::render::view::VisibilitySystems::VisibilityPropagate
                .before(semantics)
                .before(interface_labels),
        );
        #[cfg(target_arch = "wasm32")]
        app.add_systems(
            PreUpdate,
            crate::web_accessibility::input
                .before(assistive_panel)
                .before(keyboard),
        )
        .add_systems(
            PostUpdate,
            crate::web_accessibility::render
                .after(semantics)
                .after(interface_labels)
                .after(bevy::transform::TransformSystem::TransformPropagate),
        );
    }
}
fn french(locale: &Settings) -> bool {
    locale.language.starts_with("fr")
}
fn translated<'a>(fr: bool, a: &'a str, b: &'a str) -> &'a str {
    if fr {
        a
    } else {
        b
    }
}
fn initialize(runtime: &mut Accessibility, persistent: &PersistentDataResource) {
    if runtime.initialized {
        return;
    }
    runtime.player = persistent
        .data
        .accessibility
        .clone()
        .filter(|settings| settings.validate().is_ok());
    runtime.settings = runtime
        .player
        .clone()
        .unwrap_or_else(|| runtime.authored.clone());
    runtime.initialized = true;
}
fn say(runtime: &mut Accessibility, speech: &Speech, text: String) {
    let text = rvn_core::parse_text_tags(&text)
        .map(|rich| rich.plain_text())
        .unwrap_or(text);
    if text.trim().is_empty() {
        return;
    }
    let mut settings = runtime.settings.clone();
    if settings.language.is_none() && !runtime.language.is_empty() {
        settings.language = Some(runtime.language.clone());
    }
    if let Err(problem) = speech.request(&settings, SpeechRequest::Speak(text)) {
        runtime.status = problem;
        runtime.redraw = true;
    }
}
fn legacy_notice(fr: bool) -> &'static str {
    translated(fr,"Ancienne sauvegarde chargée. Sa compatibilité avec une histoire modifiée ne peut pas être vérifiée. Conservez votre sauvegarde d’origine.","Older save loaded. Compatibility with an edited story cannot be verified. Keep your original save.")
}
fn receive(
    mut events: EventReader<VnCommand>,
    mut runtime: ResMut<Accessibility>,
    persistent: Res<PersistentDataResource>,
    speech: NonSend<Speech>,
    locale: Res<Settings>,
    characters: Res<crate::resources::CharacterRegistry>,
    time: Res<Time>,
) {
    initialize(&mut runtime, &persistent);
    if runtime.language != locale.language {
        runtime.language = locale.language.clone();
        runtime.redraw = true;
    }
    for event in events.read() {
        match event {
            VnCommand::LoadedCompatibility(compatibility) => {
                runtime.legacy_notice_until =
                    if *compatibility == rvn_core::LoadCompatibility::LegacyUnchecked {
                        time.elapsed_seconds_f64() + 20.0
                    } else {
                        0.0
                    };
                if *compatibility == rvn_core::LoadCompatibility::LegacyUnchecked
                    && runtime.settings.self_voicing
                {
                    say(&mut runtime, &speech, legacy_notice(french(&locale)).into());
                }
            }
            VnCommand::Accessibility(settings) => {
                runtime.authored = settings.clone();
                let next = runtime.player.clone().unwrap_or_else(|| settings.clone());
                if next != runtime.settings {
                    runtime.settings = next;
                    runtime.redraw = true;
                    if !runtime.settings.self_voicing {
                        let _ = speech.request(&runtime.settings, SpeechRequest::Stop);
                    }
                }
            }
            VnCommand::Speech(SpeechRequest::Speak(text)) => {
                say(&mut runtime, &speech, text.clone())
            }
            VnCommand::Speech(SpeechRequest::Stop) => {
                let _ = speech.request(&runtime.settings, SpeechRequest::Stop);
            }
            VnCommand::ShowDialogue { character, text } => {
                runtime.dialogue_scroll = 0.0;
                runtime.last_dialogue = character
                    .as_ref()
                    .map(|id| characters.display_name(id))
                    .filter(|name| !name.is_empty())
                    .map(|name| format!("{name}. {text}"))
                    .unwrap_or_else(|| text.clone());
                if runtime.settings.self_voicing {
                    let text = runtime.last_dialogue.clone();
                    say(&mut runtime, &speech, text);
                }
            }
            VnCommand::ShowChoice { options } if runtime.settings.self_voicing => say(
                &mut runtime,
                &speech,
                options
                    .iter()
                    .enumerate()
                    .map(|(i, text)| format!("{}. {text}", i + 1))
                    .collect::<Vec<_>>()
                    .join(". "),
            ),
            _ => {}
        }
    }
}
fn apply_player(
    runtime: &mut Accessibility,
    persistent: &mut PersistentDataResource,
    speech: &Speech,
) {
    runtime.player = Some(runtime.settings.clone());
    persistent.data.accessibility = runtime.player.clone();
    if let Err(problem) = persistent.manager.save(&persistent.data) {
        runtime.status = format!("Could not save accessibility preferences: {problem}");
    }
    runtime.redraw = true;
    if !runtime.settings.self_voicing {
        let _ = speech.request(&runtime.settings, SpeechRequest::Stop);
    }
}
fn action(
    runtime: &mut Accessibility,
    row: usize,
    direction: f32,
    persistent: &mut PersistentDataResource,
    speech: &Speech,
) {
    match row {
        0 => runtime.settings.self_voicing = !runtime.settings.self_voicing,
        1 => {
            runtime.settings.text_scale =
                (runtime.settings.text_scale + 0.25 * direction).clamp(0.75, 2.5)
        }
        2 => runtime.settings.high_contrast = !runtime.settings.high_contrast,
        3 => runtime.settings.reduced_motion = !runtime.settings.reduced_motion,
        4 => {
            runtime.settings.speech_rate =
                (runtime.settings.speech_rate + 0.1 * direction).clamp(0.5, 2.0)
        }
        5 => {
            runtime.settings.speech_volume =
                (runtime.settings.speech_volume + 0.1 * direction).clamp(0.0, 1.0)
        }
        6 => {
            let text = runtime.last_dialogue.clone();
            say(runtime, speech, text);
            return;
        }
        7 => {
            let _ = speech.request(&runtime.settings, SpeechRequest::Stop);
            return;
        }
        8 => {
            runtime.player = None;
            persistent.data.accessibility = None;
            runtime.settings = runtime.authored.clone();
            if let Err(problem) = persistent.manager.save(&persistent.data) {
                runtime.status = format!("Could not save accessibility preferences: {problem}");
            }
            if !runtime.settings.self_voicing {
                let _ = speech.request(&runtime.settings, SpeechRequest::Stop);
            }
            runtime.redraw = true;
            return;
        }
        _ => {
            runtime.open = false;
            runtime.redraw = true;
            return;
        }
    }
    apply_player(runtime, persistent, speech);
}
fn keyboard(
    mut runtime: ResMut<Accessibility>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut interactions: Query<(&mut Interaction, Option<&PanelButton>)>,
    mut persistent: ResMut<PersistentDataResource>,
    speech: NonSend<Speech>,
    screens: Res<crate::programmable_ui::Screens>,
    locale: Res<Settings>,
) {
    initialize(&mut runtime, &persistent);
    let was_open = runtime.open;
    if keys.just_pressed(KeyCode::F8) {
        runtime.open = !runtime.open;
        runtime.redraw = true;
    }
    if runtime.open {
        if keys.just_pressed(KeyCode::Escape) {
            runtime.open = false;
            runtime.redraw = true;
        } else if keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::ArrowDown) {
            runtime.focus = (runtime.focus
                + if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                    9
                } else {
                    1
                })
                % 10;
            if runtime.settings.self_voicing {
                let text = row_label(runtime.focus, &runtime.settings, french(&locale));
                say(&mut runtime, &speech, text);
            }
        } else if keys.just_pressed(KeyCode::ArrowUp) {
            runtime.focus = (runtime.focus + 9) % 10;
            if runtime.settings.self_voicing {
                let text = row_label(runtime.focus, &runtime.settings, french(&locale));
                say(&mut runtime, &speech, text);
            }
        } else if keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::Space)
            || keys.just_pressed(KeyCode::ArrowRight)
            || keys.just_pressed(KeyCode::ArrowLeft)
        {
            let row = runtime.focus;
            let direction = if keys.just_pressed(KeyCode::ArrowLeft) {
                -1.0
            } else {
                1.0
            };
            action(&mut runtime, row, direction, &mut persistent, &speech);
        }
        if runtime.settings.self_voicing && runtime.redraw {
            let text = row_label(runtime.focus, &runtime.settings, french(&locale));
            say(&mut runtime, &speech, text);
        }
    } else if !was_open && keys.just_pressed(KeyCode::KeyV) {
        let editing = screens.keyboard_focus.as_ref().is_some_and(|(screen, id)| {
            screens
                .views
                .iter()
                .find(|view| &view.name == screen)
                .and_then(|view| view.root.find(id))
                .is_some_and(|control| control.kind == rvn_ui::programmable::ComponentKind::Input)
        });
        if !editing {
            runtime.settings.self_voicing = !runtime.settings.self_voicing;
            apply_player(&mut runtime, &mut persistent, &speech);
            if runtime.settings.self_voicing {
                let text = runtime.last_dialogue.clone();
                say(&mut runtime, &speech, text);
            }
            keys.clear_just_pressed(KeyCode::KeyV);
        }
    }
    if !was_open && !runtime.open && !screens.active && runtime.dialogue_overflow > 0.0 {
        let direction = if keys.just_pressed(KeyCode::PageDown) {
            1.0
        } else if keys.just_pressed(KeyCode::PageUp) {
            -1.0
        } else {
            0.0
        };
        if direction != 0.0 {
            runtime.dialogue_scroll =
                (runtime.dialogue_scroll + 120.0 * direction).clamp(0.0, runtime.dialogue_overflow);
            keys.clear_just_pressed(KeyCode::PageDown);
            keys.clear_just_pressed(KeyCode::PageUp);
        }
    }
    if was_open || runtime.open {
        keys.clear();
        // Clear stale button activations underneath the modal panel before any
        // story/menu system runs. The panel's own pointer controls remain live.
        for (mut interaction, panel) in &mut interactions {
            if panel.is_none() {
                *interaction = Interaction::None;
            }
        }
    }
    runtime.blocked = was_open || runtime.open;
}
fn buttons(
    mut runtime: ResMut<Accessibility>,
    buttons: Query<(&PanelButton, &Interaction), Changed<Interaction>>,
    mut persistent: ResMut<PersistentDataResource>,
    speech: NonSend<Speech>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: EventReader<bevy::input::mouse::MouseWheel>,
    mut pointer: EventReader<bevy::window::CursorMoved>,
) {
    let moved = pointer.read().any(|movement| {
        movement
            .delta
            .is_some_and(|delta| delta.length_squared() > 0.0)
    });
    if !runtime.open {
        return;
    }
    for movement in wheel.read() {
        if movement.y != 0.0 {
            runtime.focus = (runtime.focus + if movement.y < 0.0 { 1 } else { 9 }) % 10;
        }
    }
    for (button, interaction) in &buttons {
        // Rebuilding/scaling the panel beneath a stationary pointer must not
        // steal keyboard focus. A real pointer move or press does change it.
        if *interaction == Interaction::Hovered && moved {
            runtime.focus = button.0;
        }
        if *interaction == Interaction::Pressed && mouse.just_pressed(MouseButton::Left) {
            runtime.focus = button.0;
            action(&mut runtime, button.0, 1.0, &mut persistent, &speech);
        }
    }
}
fn assistive_panel(
    mut requests: EventReader<bevy::a11y::ActionRequest>,
    mut runtime: ResMut<Accessibility>,
    mut buttons: Query<
        (
            &mut Interaction,
            Option<&PanelButton>,
            Option<&bevy::a11y::AccessibilityNode>,
            Option<&InheritedVisibility>,
            Option<&crate::menu_documents::MenuFocus>,
            Option<&crate::components::ChoiceButton>,
        ),
        Without<crate::programmable_ui::Control>,
    >,
    mut focus: ResMut<bevy::a11y::Focus>,
    mut menus: ResMut<crate::menu_documents::Menus>,
    mut choice: ResMut<crate::resources::ChoiceFocus>,
    mut persistent: ResMut<PersistentDataResource>,
    speech: NonSend<Speech>,
) {
    use bevy::a11y::accesskit::Action;
    for request in requests.read() {
        let Ok(entity) = Entity::try_from_bits(request.0.target.0) else {
            continue;
        };
        let Ok((mut interaction, panel, node, visible, menu_key, choice_button)) =
            buttons.get_mut(entity)
        else {
            continue;
        };
        if visible.is_some_and(|visible| !visible.get())
            || node.is_some_and(|node| {
                let node = node.0.clone().build();
                node.is_disabled() || node.is_hidden()
            })
        {
            continue;
        }
        if let Some(panel) = panel {
            if !runtime.open {
                continue;
            }
            if request.0.action == Action::Focus {
                runtime.focus = panel.0;
            } else if request.0.action == Action::Default {
                action(&mut runtime, panel.0, 1.0, &mut persistent, &speech);
                runtime.blocked = true;
            }
        } else if !runtime.blocked {
            // Legacy menu buttons share Bevy's button semantics. They retain
            // their existing handlers, including confirmation and validation.
            if request.0.action == Action::Focus {
                if let Some(key) = menu_key {
                    menus.assistive_focus(entity, key);
                }
                if let Some(button) = choice_button {
                    choice.0 = Some(button.0);
                }
                focus.0 = Some(entity);
            } else if request.0.action == Action::Default {
                *interaction = Interaction::Pressed;
            }
        }
    }
}
fn panel_focus(
    runtime: Res<Accessibility>,
    mut buttons: Query<(
        &PanelButton,
        &Interaction,
        &mut BorderColor,
        &mut BackgroundColor,
    )>,
) {
    for (button, interaction, mut border, mut background) in &mut buttons {
        border.0 = if runtime.focus == button.0 {
            Color::srgb(0.15, 0.8, 1.0)
        } else {
            Color::srgb(0.18, 0.25, 0.32)
        };
        background.0 = match interaction {
            Interaction::Pressed => Color::srgb(0.06, 0.32, 0.42),
            Interaction::Hovered => Color::srgb(0.08, 0.22, 0.30),
            _ => Color::srgb(0.06, 0.12, 0.18),
        };
    }
}
fn panel_scroll(
    runtime: Res<Accessibility>,
    viewport: Query<&Node, With<PanelViewport>>,
    mut rows: Query<(&Node, &mut Style), With<PanelRows>>,
) {
    let Ok(viewport) = viewport.get_single() else {
        return;
    };
    let Ok((content, mut style)) = rows.get_single_mut() else {
        return;
    };
    if viewport.size().y <= 0.0 || content.size().y <= 0.0 {
        return;
    }
    let row_height = 44.0 * runtime.settings.text_scale + 5.0;
    let previous = match style.top {
        Val::Px(value) => -value,
        _ => 0.0,
    };
    let low = runtime.focus as f32 * row_height;
    let high = low + row_height;
    let offset = if low < previous {
        low
    } else if high > previous + viewport.size().y {
        high - viewport.size().y
    } else {
        previous
    };
    let next = Val::Px(-offset.clamp(0.0, (content.size().y - viewport.size().y).max(0.0)));
    if style.top != next {
        style.top = next;
    }
}
fn notice_text(runtime: &Accessibility, fr: bool, now: f64) -> String {
    let mut text = if runtime.open || runtime.status.is_empty() {
        String::new()
    } else {
        format!(
            "{}\n{}",
            runtime.status,
            translated(
                fr,
                "F8 : réglages d’accessibilité",
                "F8: accessibility settings"
            )
        )
    };
    if !runtime.open && now < runtime.legacy_notice_until {
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(legacy_notice(fr));
    }
    text
}
fn notice(
    mut commands: Commands,
    runtime: Res<Accessibility>,
    notices: Query<Entity, With<VoiceNotice>>,
    font: Res<crate::menu_documents::MenuFont>,
    locale: Res<Settings>,
    mut last: Local<String>,
    time: Res<Time>,
) {
    let text = notice_text(&runtime, french(&locale), time.elapsed_seconds_f64());
    if *last == text {
        return;
    }
    *last = text.clone();
    for entity in &notices {
        commands.entity(entity).despawn_recursive();
    }
    if text.is_empty() {
        return;
    }
    commands
        .spawn((
            VoiceNotice,
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(12.0),
                    left: Val::Px(12.0),
                    max_width: Val::Percent(95.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    ..default()
                },
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.95).into(),
                z_index: ZIndex::Global(12001),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                VoiceNoticeText,
                TextBundle::from_section(
                    text,
                    TextStyle {
                        font: font.0.clone(),
                        font_size: 18.0,
                        color: Color::srgb(1.0, 0.85, 0.45),
                    },
                ),
            ));
        });
}
fn focused(
    mut runtime: ResMut<Accessibility>,
    screens: Res<crate::programmable_ui::Screens>,
    speech: NonSend<Speech>,
    focus: Res<bevy::a11y::Focus>,
    nodes: Query<&bevy::a11y::AccessibilityNode>,
) {
    if runtime.open {
        return;
    }
    if !screens.active || screens.keyboard_focus.is_none() {
        if runtime.last_native_focus != focus.0 {
            runtime.last_native_focus = focus.0;
            if runtime.settings.self_voicing {
                if let Some(name) = focus
                    .0
                    .and_then(|entity| nodes.get(entity).ok())
                    .and_then(|node| node.0.clone().build().name().map(str::to_owned))
                {
                    say(&mut runtime, &speech, name);
                }
            }
        }
        return;
    }
    if runtime.last_focus == screens.keyboard_focus {
        return;
    }
    runtime.last_focus = screens.keyboard_focus.clone();
    if !runtime.settings.self_voicing {
        return;
    }
    let Some((screen, id)) = &screens.keyboard_focus else {
        return;
    };
    if let Some(control) = screens
        .views
        .iter()
        .find(|view| &view.name == screen)
        .and_then(|view| view.root.find(id))
    {
        let name = control.accessible_label.clone().unwrap_or_else(|| {
            if control.text.is_empty() {
                control.placeholder.clone()
            } else {
                control.text.clone()
            }
        });
        let value = match &control.value {
            serde_json::Value::Null => String::new(),
            serde_json::Value::String(value) if value.is_empty() => String::new(),
            serde_json::Value::String(value) => format!(". {value}"),
            other => format!(". {other}"),
        };
        say(&mut runtime, &speech, format!("{name}{value}"));
    }
}
fn voice_status(mut runtime: ResMut<Accessibility>, speech: NonSend<Speech>) {
    while let Some(result) = speech.report() {
        runtime.status = result.err().unwrap_or_default();
        runtime.redraw = true;
    }
}
fn interface_control_role(
    kind: rvn_ui::programmable::ComponentKind,
    is_dropdown_option: bool,
) -> bevy::a11y::accesskit::Role {
    use bevy::a11y::accesskit::Role;
    use rvn_ui::programmable::ComponentKind as Kind;
    if is_dropdown_option {
        return Role::Button;
    }
    match kind {
        Kind::Input => Role::TextInput,
        Kind::Select => Role::ComboBox,
        Kind::Toggle => Role::CheckBox,
        Kind::Slider => Role::Slider,
        Kind::Canvas => Role::Canvas,
        _ => Role::Button,
    }
}
fn semantics(
    mut commands: Commands,
    runtime: Res<Accessibility>,
    screens: Res<crate::programmable_ui::Screens>,
    controls: Query<(Entity, &crate::programmable_ui::Control)>,
    panel: Query<(Entity, &PanelButton)>,
    mut focus: ResMut<bevy::a11y::Focus>,
    buttons: Query<
        (
            Entity,
            Option<&Children>,
            Option<&PanelButton>,
            Option<&InheritedVisibility>,
        ),
        (With<Button>, Without<crate::programmable_ui::Control>),
    >,
    texts: Query<&Text>,
    children: Query<&Children>,
    labels: Query<
        (Entity, &Text, Option<&crate::components::DialogueText>),
        (
            Or<(With<crate::components::DialogueText>, With<VoiceNoticeText>)>,
            Without<Button>,
        ),
    >,
    menus: Res<crate::menu_documents::Menus>,
    choice: Res<crate::resources::ChoiceFocus>,
    choices: Query<(Entity, &crate::components::ChoiceButton)>,
    geometry: Query<(&Node, &GlobalTransform)>,
) {
    use bevy::a11y::{
        accesskit::{Action, NodeBuilder, Role, Toggled},
        AccessibilityNode,
    };
    let mut target = None;
    let bounds = |entity: Entity, node: &mut NodeBuilder| {
        if let Ok((layout, transform)) = geometry.get(entity) {
            let center = transform.translation().truncate();
            let size = layout.size() * 0.5;
            node.set_bounds(bevy::a11y::accesskit::Rect::new(
                f64::from(center.x - size.x),
                f64::from(center.y - size.y),
                f64::from(center.x + size.x),
                f64::from(center.y + size.y),
            ));
        }
    };
    fn name(
        entity: Entity,
        texts: &Query<&Text>,
        children: &Query<&Children>,
        output: &mut String,
    ) {
        if let Ok(text) = texts.get(entity) {
            for section in &text.sections {
                output.push_str(&section.value);
            }
            output.push(' ');
        }
        if let Ok(children_list) = children.get(entity) {
            for child in children_list {
                name(*child, texts, children, output);
            }
        }
    }
    for (entity, _, panel, visible) in &buttons {
        let mut label = String::new();
        name(entity, &texts, &children, &mut label);
        let mut node = NodeBuilder::new(Role::Button);
        node.set_name(label.trim().to_owned());
        if visible.is_some_and(|visible| !visible.get()) || (runtime.open && panel.is_none()) {
            node.set_hidden();
            node.set_disabled();
        } else {
            node.add_action(Action::Focus);
            node.add_action(Action::Default);
        }
        bounds(entity, &mut node);
        commands
            .entity(entity)
            .insert(AccessibilityNode::from(node));
    }
    for (entity, text, dialogue) in &labels {
        let mut node = NodeBuilder::new(Role::StaticText);
        node.set_name(if dialogue.is_some() {
            rvn_core::parse_text_tags(&runtime.last_dialogue)
                .map(|rich| rich.plain_text())
                .unwrap_or_else(|_| runtime.last_dialogue.clone())
        } else {
            text.sections
                .iter()
                .map(|section| section.value.as_str())
                .collect::<String>()
        });
        if runtime.open
            || geometry
                .get(entity)
                .is_ok_and(|(layout, _)| layout.size() == Vec2::ZERO)
        {
            node.set_hidden();
        }
        bounds(entity, &mut node);
        commands
            .entity(entity)
            .insert(AccessibilityNode::from(node));
    }
    for (entity, control) in &controls {
        let Some(view) = screens
            .views
            .iter()
            .find(|view| view.name == control.screen)
        else {
            continue;
        };
        let Some(component) = view.root.find(&control.element) else {
            continue;
        };
        use rvn_ui::programmable::ComponentKind as Kind;
        let role = interface_control_role(component.kind, control.option.is_some());
        let mut node = NodeBuilder::new(role);
        node.set_name(if let Some(option) = &control.option {
            component
                .options
                .iter()
                .position(|value| value == option)
                .and_then(|index| component.option_labels.get(index))
                .unwrap_or(option)
                .clone()
        } else {
            component
                .accessible_label
                .as_deref()
                .filter(|name| !name.is_empty())
                .unwrap_or(if component.text.is_empty() {
                    &component.placeholder
                } else {
                    &component.text
                })
                .to_string()
        });
        node.set_value(
            component
                .value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    if component.value.is_null() {
                        String::new()
                    } else {
                        component.value.to_string()
                    }
                }),
        );
        let covered = runtime.open
            || screens
                .views
                .iter()
                .any(|other| other.modal && (other.layer, other.order) > (view.layer, view.order));
        let disabled = covered || !view.root.available(&component.id);
        if covered {
            node.set_hidden();
        }
        if disabled {
            node.set_disabled();
        } else {
            node.add_action(Action::Focus);
            if component.kind == Kind::Canvas {
                if component
                    .events
                    .contains_key(&rvn_ui::programmable::ScreenEventKind::Activate)
                {
                    node.add_action(Action::Default);
                }
            } else {
                node.add_action(
                    if control.option.is_some()
                        || matches!(component.kind, Kind::Button | Kind::Toggle | Kind::Select)
                    {
                        Action::Default
                    } else {
                        Action::SetValue
                    },
                );
                if control.option.is_none() && matches!(component.kind, Kind::Select | Kind::Slider)
                {
                    node.add_action(Action::SetValue);
                }
            }
        }
        if component.kind == Kind::Toggle {
            node.set_toggled(if component.value.as_bool().unwrap_or(false) {
                Toggled::True
            } else {
                Toggled::False
            });
        }
        if component.kind == Kind::Slider {
            node.set_numeric_value(component.value.as_f64().unwrap_or(0.0));
            node.set_min_numeric_value(component.min);
            node.set_max_numeric_value(component.max);
            node.set_numeric_value_step((component.max - component.min) / 100.0);
        }
        bounds(entity, &mut node);
        commands
            .entity(entity)
            .insert(AccessibilityNode::from(node));
        if !disabled
            && screens
                .keyboard_focus
                .as_ref()
                .is_some_and(|(screen, id)| screen == &control.screen && id == &control.element)
        {
            target = Some(entity);
        }
    }
    if runtime.open {
        target = panel
            .iter()
            .find(|(_, button)| button.0 == runtime.focus)
            .map(|(entity, _)| entity);
    }
    if target.is_none() && !runtime.open && !screens.active {
        target = menus.keyboard_focus().or_else(|| {
            choice.0.and_then(|index| {
                choices
                    .iter()
                    .find(|(_, button)| button.0 == index)
                    .map(|(entity, _)| entity)
            })
        });
    }
    if target.is_some() || screens.active || runtime.open {
        focus.0 = target;
    }
}
fn interface_labels(
    mut commands: Commands,
    runtime: Res<Accessibility>,
    screens: Res<crate::programmable_ui::Screens>,
    targets: Query<
        (
            Entity,
            &crate::composed_motion::InterfaceTarget,
            &Node,
            &GlobalTransform,
            Option<&InheritedVisibility>,
        ),
        Without<crate::programmable_ui::Control>,
    >,
) {
    use bevy::a11y::{
        accesskit::{NodeBuilder, Role},
        AccessibilityNode,
    };
    for (entity, target, layout, transform, visible) in &targets {
        let Some(view) = screens.views.iter().find(|view| view.name == target.screen) else {
            continue;
        };
        let Some(component) = view.root.find(&target.element) else {
            continue;
        };
        let role = match component.kind {
            rvn_ui::programmable::ComponentKind::Text => Role::StaticText,
            rvn_ui::programmable::ComponentKind::Image => Role::Image,
            _ => continue,
        };
        let name = component
            .accessible_label
            .as_deref()
            .filter(|name| !name.is_empty())
            .unwrap_or(if role == Role::StaticText {
                &component.text
            } else {
                ""
            });
        if name.is_empty() {
            commands.entity(entity).remove::<AccessibilityNode>();
            continue;
        }
        let mut node = NodeBuilder::new(role);
        node.set_name(name.to_owned());
        let covered = screens
            .views
            .iter()
            .any(|other| other.modal && (other.layer, other.order) > (view.layer, view.order));
        if runtime.open || covered || visible.is_some_and(|visible| !visible.get()) {
            node.set_hidden();
        }
        let center = transform.translation().truncate();
        let half = layout.size() * 0.5;
        node.set_bounds(bevy::a11y::accesskit::Rect::new(
            f64::from(center.x - half.x),
            f64::from(center.y - half.y),
            f64::from(center.x + half.x),
            f64::from(center.y + half.y),
        ));
        commands
            .entity(entity)
            .insert(AccessibilityNode::from(node));
    }
}
fn row_label(index: usize, settings: &AccessibilitySettings, fr: bool) -> String {
    let boolean = |value| {
        translated(
            fr,
            if value { "Activé" } else { "Désactivé" },
            if value { "On" } else { "Off" },
        )
    };
    match index {
        0 => format!(
            "{} : {}",
            translated(fr, "Lecture vocale", "Self-voicing"),
            boolean(settings.self_voicing)
        ),
        1 => format!(
            "{} : {:.0}%",
            translated(fr, "Taille du texte", "Text size"),
            settings.text_scale * 100.0
        ),
        2 => format!(
            "{} : {}",
            translated(fr, "Contraste élevé", "High contrast"),
            boolean(settings.high_contrast)
        ),
        3 => format!(
            "{} : {}",
            translated(fr, "Réduire les mouvements", "Reduced motion"),
            boolean(settings.reduced_motion)
        ),
        4 => format!(
            "{} : {:.1}×",
            translated(fr, "Vitesse de lecture", "Speech rate"),
            settings.speech_rate
        ),
        5 => format!(
            "{} : {:.0}%",
            translated(fr, "Volume de lecture", "Speech volume"),
            settings.speech_volume * 100.0
        ),
        6 => translated(fr, "Lire le dialogue actuel", "Read current dialogue").into(),
        7 => translated(fr, "Arrêter la lecture", "Stop speech").into(),
        8 => translated(
            fr,
            "Rétablir les réglages du projet",
            "Reset to project settings",
        )
        .into(),
        _ => translated(fr, "Fermer (Échap / F8)", "Close (Escape / F8)").into(),
    }
}
fn panel(
    mut commands: Commands,
    mut runtime: ResMut<Accessibility>,
    roots: Query<Entity, With<PanelRoot>>,
    font: Res<crate::menu_documents::MenuFont>,
    locale: Res<Settings>,
    windows: Query<&Window>,
    mut last_size: Local<Vec2>,
) {
    let size = windows
        .get_single()
        .map(|window| Vec2::new(window.width(), window.height()))
        .unwrap_or(Vec2::new(1280.0, 720.0));
    if *last_size != size {
        *last_size = size;
        runtime.redraw = true;
    }
    if !runtime.redraw {
        return;
    }
    runtime.redraw = false;
    for entity in &roots {
        commands.entity(entity).despawn_recursive();
    }
    if !runtime.open {
        return;
    }
    let fr = french(&locale);
    let scale = runtime.settings.text_scale;
    let height = (size.y - 24.0).max(100.0).min(570.0 * scale);
    commands
        .spawn((
            PanelRoot,
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.88).into(),
                focus_policy: bevy::ui::FocusPolicy::Block,
                z_index: ZIndex::Global(12000),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(660.0),
                    max_width: Val::Percent(95.0),
                    height: Val::Px(height),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(5.0),
                    ..default()
                },
                background_color: Color::srgb(0.025, 0.04, 0.06).into(),
                ..default()
            })
            .with_children(|body| {
                body.spawn((
                    PanelText,
                    TextBundle::from_section(
                        translated(fr, "Accessibilité", "Accessibility"),
                        TextStyle {
                            font: font.0.clone(),
                            font_size: 24.0 * scale,
                            color: Color::WHITE,
                        },
                    ),
                ));
                body.spawn((
                    PanelText,
                    TextBundle::from_section(
                        translated(
                            fr,
                            "Tab / ↑ ↓ : naviguer · ← → : régler · Entrée : activer",
                            "Tab / ↑ ↓: navigate · ← →: adjust · Enter: activate",
                        ),
                        TextStyle {
                            font: font.0.clone(),
                            font_size: 14.0 * scale,
                            color: Color::srgb(0.7, 0.8, 0.9),
                        },
                    ),
                ));
                body.spawn((
                    PanelViewport,
                    NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            flex_grow: 1.0,
                            flex_basis: Val::Px(0.0),
                            min_height: Val::Px(20.0),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        ..default()
                    },
                ))
                .with_children(|viewport| {
                    viewport
                        .spawn((
                            PanelRows,
                            NodeBundle {
                                style: Style {
                                    position_type: PositionType::Absolute,
                                    width: Val::Percent(100.0),
                                    height: Val::Px(10.0 * (44.0 * scale + 5.0) - 5.0),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(5.0),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                ..default()
                            },
                        ))
                        .with_children(|body| {
                            for index in 0..10 {
                                body.spawn((
                                    PanelButton(index),
                                    ButtonBundle {
                                        style: Style {
                                            width: Val::Percent(100.0),
                                            height: Val::Px(44.0 * scale),
                                            flex_shrink: 0.0,
                                            padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        border_color: if runtime.focus == index {
                                            Color::srgb(0.15, 0.8, 1.0).into()
                                        } else {
                                            Color::srgb(0.18, 0.25, 0.32).into()
                                        },
                                        background_color: Color::srgb(0.06, 0.12, 0.18).into(),
                                        ..default()
                                    },
                                ))
                                .with_children(|button| {
                                    button.spawn((
                                        PanelText,
                                        TextBundle::from_section(
                                            row_label(index, &runtime.settings, fr),
                                            TextStyle {
                                                font: font.0.clone(),
                                                font_size: 16.0 * scale,
                                                color: Color::WHITE,
                                            },
                                        ),
                                    ));
                                });
                            }
                        });
                });
                if !runtime.status.is_empty() {
                    body.spawn((
                        PanelText,
                        TextBundle::from_section(
                            &runtime.status,
                            TextStyle {
                                font: font.0.clone(),
                                font_size: 14.0 * scale,
                                color: Color::srgb(1.0, 0.75, 0.4),
                            },
                        ),
                    ));
                }
            });
        });
}
fn visuals(
    mut commands: Commands,
    runtime: Res<Accessibility>,
    mut texts: Query<(Entity, &mut Text, Option<&mut OriginalText>), Without<PanelText>>,
    mut backgrounds: Query<
        (
            Entity,
            &mut BackgroundColor,
            Option<&mut OriginalBackground>,
        ),
        With<Node>,
    >,
) {
    for (entity, mut text, original) in &mut texts {
        let Some(mut original) = original else {
            commands.entity(entity).insert(OriginalText(
                text.sections
                    .iter()
                    .map(|section| {
                        (
                            section.style.font_size,
                            section.style.color,
                            section.style.font_size,
                            section.style.color,
                        )
                    })
                    .collect(),
            ));
            continue;
        };
        while original.0.len() < text.sections.len() {
            let section = &text.sections[original.0.len()];
            original.0.push((
                section.style.font_size,
                section.style.color,
                section.style.font_size,
                section.style.color,
            ));
        }
        for (section, (size, color, last_size, last_color)) in
            text.sections.iter_mut().zip(&mut original.0)
        {
            if section.style.font_size != *last_size {
                *size = section.style.font_size;
            }
            if section.style.color != *last_color {
                *color = section.style.color;
            }
            let next_size = *size * runtime.settings.text_scale;
            let next_color = if runtime.settings.high_contrast {
                Color::WHITE
            } else {
                *color
            };
            if section.style.font_size != next_size {
                section.style.font_size = next_size;
            }
            if section.style.color != next_color {
                section.style.color = next_color;
            }
            *last_size = next_size;
            *last_color = next_color;
        }
    }
    for (entity, mut background, original) in &mut backgrounds {
        let Some(mut original) = original else {
            commands
                .entity(entity)
                .insert(OriginalBackground(background.0, background.0));
            continue;
        };
        if background.0 != original.1 {
            original.0 = background.0;
        }
        let next = if runtime.settings.high_contrast && original.0.alpha() > 0.0 {
            Color::BLACK
        } else {
            original.0
        };
        if background.0 != next {
            background.0 = next;
        }
        original.1 = next;
    }
}
// Typewriter redraws copy the preceding style. Restore its authored font
// before it creates new rich-text sections, rather than multiplying an already
// enlarged font once again each time a new segment appears.
fn reset_text(
    mut texts: Query<(&mut Text, &OriginalText)>,
    typewriter: Option<Res<crate::resources::TypewriterState>>,
) {
    // Settled dialogue does not need a style reset every frame. Avoid marking
    // text/layout dirty while the player is simply reading enlarged text.
    if typewriter.is_some_and(|state| !state.typing) {
        return;
    }
    for (mut text, original) in &mut texts {
        for (section, (size, color, last_size, last_color)) in
            text.sections.iter_mut().zip(&original.0)
        {
            if section.style.font_size == *last_size && section.style.font_size != *size {
                section.style.font_size = *size;
            }
            if section.style.color == *last_color && section.style.color != *color {
                section.style.color = *color;
            }
        }
    }
}
fn dialogue_layout(
    mut runtime: ResMut<Accessibility>,
    theme: Res<crate::resources::Theme>,
    windows: Query<&Window>,
    mut boxes: Query<(&Children, &mut Style), With<crate::components::DialogueBox>>,
    mut content: Query<
        (&bevy::text::TextLayoutInfo, &mut Style),
        Without<crate::components::DialogueBox>,
    >,
) {
    let available = windows.get_single().map_or(720.0, |window| window.height()) - 24.0;
    for (children, mut style) in &mut boxes {
        let measured = children
            .iter()
            .filter_map(|child| content.get(*child).ok())
            .map(|(layout, _)| layout.logical_size.y)
            .sum::<f32>();
        let desired = (measured + 2.0 * theme.textbox.padding)
            .max(theme.textbox.height * runtime.settings.text_scale.max(1.0));
        let height = Val::Px(desired.min(available.max(100.0)));
        runtime.dialogue_overflow = (desired - available.max(100.0)).max(0.0);
        runtime.dialogue_scroll = runtime
            .dialogue_scroll
            .clamp(0.0, runtime.dialogue_overflow);
        if style.height != height {
            style.height = height;
        }
        if style.overflow != Overflow::clip() {
            style.overflow = Overflow::clip();
        }
        for child in children {
            if let Ok((_, mut child_style)) = content.get_mut(*child) {
                if child_style.flex_shrink != 0.0 {
                    child_style.flex_shrink = 0.0;
                }
                let top = Val::Px(-runtime.dialogue_scroll);
                if child_style.top != top {
                    child_style.top = top;
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn install_qa(app: &mut App) {
    app.init_resource::<QaKeys>()
        .add_systems(
            PreUpdate,
            qa_keys
                .after(bevy::input::InputSystem)
                .before(AccessibilityInput),
        )
        .add_systems(Last, qa_drive);
}
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Default)]
struct QaKeys(Option<KeyCode>);
#[cfg(not(target_arch = "wasm32"))]
fn qa_keys(mut pending: ResMut<QaKeys>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    keys.reset_all();
    if let Some(key) = pending.0.take() {
        keys.press(key);
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn qa_drive(world: &mut World) {
    use crate::resources::{VnEngine, VnState};
    #[derive(Resource)]
    struct Run {
        started: std::time::Instant,
        entered: f64,
        step: usize,
        pc: usize,
    }
    if !world.contains_resource::<Run>() {
        world.insert_resource(Run {
            started: std::time::Instant::now(),
            entered: 0.0,
            step: 0,
            pc: 0,
        });
    }
    let now = world.resource::<Run>().started.elapsed().as_secs_f64();
    let step = world.resource::<Run>().step;
    assert!(now < 90.0, "Accessibility QA timed out at step {step}");
    assert_ne!(
        *world.resource::<State<VnState>>().get(),
        VnState::Error,
        "{}",
        world.resource::<crate::resources::ScriptErrorMessage>().0
    );
    if *world.resource::<State<VnState>>().get() == VnState::TitleScreen {
        if now > 1.0 {
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        return;
    }
    if *world.resource::<State<VnState>>().get() != VnState::Waiting
        || now - world.resource::<Run>().entered < 0.5
    {
        return;
    }
    let directory = std::path::PathBuf::from(std::env::var_os("RVN_QA_OUTPUT").unwrap());
    let window = world
        .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
        .single(world);
    let capture = |world: &mut World, name: &str| {
        world
            .resource_mut::<bevy::render::view::screenshot::ScreenshotManager>()
            .save_screenshot_to_disk(window, directory.join(name))
            .unwrap()
    };
    match step {
        0 => {
            capture(world, "01_game.png");
            let pc = world.resource::<VnEngine>().0.state.pc;
            world.resource_mut::<Run>().pc = pc;
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::F8);
        }
        1 => {
            assert!(world.resource::<Accessibility>().open);
            capture(world, "02_panel.png");
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::ArrowDown);
        }
        2..=7 => {
            assert_eq!(world.resource::<Accessibility>().focus, 1);
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::ArrowRight);
        }
        8 => {
            assert_eq!(world.resource::<Accessibility>().settings.text_scale, 2.5);
            capture(world, "03_large_panel.png");
            world
                .get_mut::<Window>(window)
                .unwrap()
                .resolution
                .set(800.0, 500.0);
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::ArrowDown);
        }
        9 => {
            assert_eq!(world.resource::<Accessibility>().focus, 2);
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::Enter);
        }
        10 => {
            assert!(world.resource::<Accessibility>().settings.high_contrast);
            capture(world, "04_small_panel.png");
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::ArrowDown);
        }
        11 => {
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::Enter);
        }
        12 => {
            assert!(world.resource::<Accessibility>().settings.reduced_motion);
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::ArrowUp);
        }
        13..=17 => {
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::ArrowDown);
        }
        18 => {
            let focus = world.resource::<Accessibility>().focus;
            let (viewport, transform) = world
                .query_filtered::<(&Node, &GlobalTransform), With<PanelViewport>>()
                .single(world);
            let low = transform.translation().y - viewport.size().y * 0.5;
            let high = transform.translation().y + viewport.size().y * 0.5;
            let (node, transform, _) = world
                .query_filtered::<(&Node, &GlobalTransform, &PanelButton), With<PanelButton>>()
                .iter(world)
                .find(|(_, _, button)| button.0 == focus)
                .unwrap();
            assert!(
                transform.translation().y - node.size().y * 0.5 >= low - 1.0
                    && transform.translation().y + node.size().y * 0.5 <= high + 1.0,
                "Focused accessibility row is clipped"
            );
            capture(world, "05_scrolled_panel.png");
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::Escape);
        }
        19 => {
            assert!(!world.resource::<Accessibility>().open);
            assert_eq!(
                world.resource::<VnEngine>().0.state.pc,
                world.resource::<Run>().pc,
                "Closing accessibility advanced the narrative"
            );
            let persistent = world.resource::<PersistentDataResource>();
            assert_eq!(
                persistent
                    .manager
                    .load()
                    .unwrap()
                    .accessibility
                    .as_ref()
                    .unwrap()
                    .text_scale,
                2.5
            );
            for text in world
                .query_filtered::<&Text, With<crate::components::DialogueText>>()
                .iter(world)
            {
                assert!(
                    text.sections
                        .iter()
                        .all(|section| section.style.font_size <= 150.0),
                    "Text size compounded"
                );
            }
            for (node,layout) in world.query_filtered::<(&Node,&bevy::text::TextLayoutInfo),With<crate::components::DialogueText>>().iter(world){assert!(layout.logical_size.y<=node.size().y+1.0,"Enlarged dialogue glyphs are clipped by an incorrectly measured node");}
            let focused = world.resource::<Accessibility>().focus;
            let content = world
                .query_filtered::<(&Node, &GlobalTransform, &PanelButton), With<PanelButton>>()
                .iter(world)
                .find(|(_, _, row)| row.0 == focused)
                .map(|(node, transform, _)| (node.size(), transform.translation()));
            let dialogue:Vec<_>=world.query_filtered::<(&Node,&GlobalTransform,&bevy::text::TextLayoutInfo),With<crate::components::DialogueText>>().iter(world).map(|(node,transform,text)|serde_json::json!({"node":[node.size().x,node.size().y],"position":[transform.translation().x,transform.translation().y],"logical":[text.logical_size.x,text.logical_size.y]})).collect();
            std::fs::write(directory.join("layout.json"),serde_json::to_vec_pretty(&serde_json::json!({"dialogue":dialogue,"focus_geometry":content.map(|(size,position)|[size.x,size.y,position.x,position.y])})).unwrap()).unwrap();
            capture(world, "06_readable_game.png");
        }
        20..=21 => {
            world.resource_mut::<QaKeys>().0 = Some(KeyCode::PageDown);
        }
        22 => {
            assert_eq!(
                world.resource::<VnEngine>().0.state.pc,
                world.resource::<Run>().pc,
                "Scrolling a dialogue advanced the narrative"
            );
            let runtime = world.resource::<Accessibility>();
            assert_eq!(
                runtime.dialogue_scroll,
                runtime.dialogue_overflow.min(240.0)
            );
            capture(world, "07_scrolled_dialogue.png");
        }
        23 => {
            let engine = &mut world.resource_mut::<VnEngine>().0;
            let mut saved = rvn_core::save::SaveData::from_state(
                &engine.state,
                1,
                "start".into(),
                "main.rvn".into(),
            );
            saved.story_identity = None;
            std::fs::write(
                directory.join("legacy-save.json"),
                serde_json::to_vec_pretty(&saved).unwrap(),
            )
            .unwrap();
            engine.load_data(saved).unwrap();
        }
        24 => {
            assert!(
                world.resource::<Accessibility>().legacy_notice_until
                    > world.resource::<Time>().elapsed_seconds_f64()
            );
            assert_eq!(
                world.resource::<VnEngine>().0.state.pc,
                world.resource::<Run>().pc
            );
            let expected = legacy_notice(french(world.resource::<Settings>()));
            assert!(
                world
                    .query_filtered::<&Text, With<VoiceNoticeText>>()
                    .iter(world)
                    .any(|text| text
                        .sections
                        .iter()
                        .any(|section| section.value.contains(expected))),
                "Legacy compatibility notice is not displayed"
            );
            capture(world, "08_legacy_save_notice.png");
            let mut saved = rvn_core::save::SaveData::from_state(
                &world.resource::<VnEngine>().0.state,
                1,
                "start".into(),
                "main.rvn".into(),
            );
            saved.story_identity = None;
            saved.pc = usize::MAX;
            assert!(world.resource_mut::<VnEngine>().0.load_data(saved).is_err());
        }
        25 => {
            assert_eq!(
                world.resource::<VnEngine>().0.state.pc,
                world.resource::<Run>().pc,
                "Refusing an invalid save changed the current story"
            );
            let engine = &mut world.resource_mut::<VnEngine>().0;
            let saved = rvn_core::save::SaveData::from_state(
                &engine.state,
                1,
                "start".into(),
                "main.rvn".into(),
            );
            // A legacy load must not fabricate a verified identity for the
            // original file. The current story identity is already known.
            assert!(saved.story_identity.is_some());
            engine.load_data(saved).unwrap();
        }
        26 => {
            assert_eq!(world.resource::<Accessibility>().legacy_notice_until, 0.0);
            let original: rvn_core::save::SaveData =
                serde_json::from_slice(&std::fs::read(directory.join("legacy-save.json")).unwrap())
                    .unwrap();
            assert!(original.story_identity.is_none());
            capture(world, "09_verified_save.png");
            std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"result":"pass","scenario":"native keyboard panel, 250 percent text, high contrast, reduced motion, 800 by 500 window, focus and dialogue scrolling, persistent preferences, modal close without narrative advance, translated legacy-save notice, invalid-load refusal and verified-load notice reset","windows_validated":false})).unwrap()).unwrap();
            world.send_event(bevy::app::AppExit::Success);
            return;
        }
        _ => unreachable!(),
    }
    let mut run = world.resource_mut::<Run>();
    run.step += 1;
    run.entered = now;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_drawing_semantics_are_canvas_not_an_implicit_button() {
        use bevy::a11y::accesskit::Role;
        use rvn_ui::programmable::ComponentKind as Kind;
        assert_eq!(interface_control_role(Kind::Canvas, false), Role::Canvas);
        assert_eq!(interface_control_role(Kind::Button, false), Role::Button);
        assert_eq!(interface_control_role(Kind::Select, true), Role::Button);
        assert_eq!(interface_control_role(Kind::Input, false), Role::TextInput);
        let canvas=rvn_ui::programmable::Component::parse(serde_json::json!({"id":"custom","kind":"canvas","accessible_label":"Interactive puzzle","events":{"activate":"activate_puzzle","key":"puzzle_key","pointer_down":"puzzle_pointer"}})).unwrap();
        assert_eq!(interface_control_role(canvas.kind, false), Role::Canvas);
    }
    #[test]
    fn interface_text_and_images_have_semantics_and_respect_modality() {
        use bevy::a11y::{accesskit::Role, AccessibilityNode};
        use rvn_ui::programmable::{Component, ScreenView};
        let root=Component::parse(serde_json::json!({"id":"root","kind":"column","children":[{"id":"quest","kind":"text","text":"Archive unlocked"},{"id":"map","kind":"image","image":"map.png","accessible_label":"Archive map"},{"id":"decoration","kind":"image","image":"frame.png"}]})).unwrap();
        let mut app = App::new();
        app.init_resource::<Accessibility>()
            .init_resource::<crate::programmable_ui::Screens>()
            .add_systems(Update, interface_labels);
        app.world_mut()
            .resource_mut::<crate::programmable_ui::Screens>()
            .views = vec![ScreenView {
            name: "journal".into(),
            modal: false,
            layer: 0,
            order: 0,
            focus: None,
            root,
        }];
        let spawn = |app: &mut App, element: &str| {
            app.world_mut()
                .spawn((
                    crate::composed_motion::InterfaceTarget {
                        screen: "journal".into(),
                        element: element.into(),
                    },
                    Node::default(),
                    GlobalTransform::default(),
                ))
                .id()
        };
        let quest = spawn(&mut app, "quest");
        let map = spawn(&mut app, "map");
        let decoration = spawn(&mut app, "decoration");
        app.update();
        let text = app
            .world()
            .get::<AccessibilityNode>(quest)
            .unwrap()
            .0
            .clone()
            .build();
        assert_eq!(text.role(), Role::StaticText);
        assert_eq!(text.name(), Some("Archive unlocked"));
        assert!(!text.is_hidden());
        assert_eq!(
            app.world()
                .get::<AccessibilityNode>(map)
                .unwrap()
                .0
                .clone()
                .build()
                .role(),
            Role::Image
        );
        assert!(app.world().get::<AccessibilityNode>(decoration).is_none());
        let puzzle = ScreenView {
            name: "puzzle".into(),
            modal: true,
            layer: 1,
            order: 1,
            focus: None,
            root: Component::parse(serde_json::json!({"id":"root","kind":"column"})).unwrap(),
        };
        app.world_mut()
            .resource_mut::<crate::programmable_ui::Screens>()
            .views
            .push(puzzle);
        app.update();
        assert!(app
            .world()
            .get::<AccessibilityNode>(quest)
            .unwrap()
            .0
            .clone()
            .build()
            .is_hidden());
        app.world_mut()
            .resource_mut::<crate::programmable_ui::Screens>()
            .views
            .pop();
        app.world_mut().resource_mut::<Accessibility>().open = true;
        app.update();
        assert!(app
            .world()
            .get::<AccessibilityNode>(map)
            .unwrap()
            .0
            .clone()
            .build()
            .is_hidden());
    }
    #[test]
    fn legacy_save_notice_expires_translates_and_keeps_voice_errors() {
        let mut runtime = Accessibility::default();
        runtime.legacy_notice_until = 20.0;
        assert_eq!(notice_text(&runtime, false, 0.0), legacy_notice(false));
        assert_eq!(notice_text(&runtime, true, 19.0), legacy_notice(true));
        assert!(notice_text(&runtime, true, 20.0).is_empty());
        runtime.status = "No voice available".into();
        assert!(notice_text(&runtime, false, 10.0).contains(legacy_notice(false)));
        assert!(notice_text(&runtime, false, 21.0).contains("No voice available"));
        runtime.open = true;
        assert!(notice_text(&runtime, false, 1.0).is_empty());
    }
    #[test]
    fn verified_load_clears_an_earlier_legacy_save_notice() {
        let directory = std::env::temp_dir().join(format!(
            "rvn-load-notice-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let manager = rvn_core::PersistentDataManager::new(&directory).unwrap();
        let mut app = App::new();
        app.init_resource::<Accessibility>()
            .init_resource::<Time>()
            .insert_resource(Settings::default())
            .insert_resource(crate::resources::CharacterRegistry(Default::default()))
            .insert_resource(PersistentDataResource {
                manager,
                data: Default::default(),
            })
            .insert_non_send_resource(Speech::default())
            .add_event::<VnCommand>()
            .add_systems(Update, receive);
        app.world_mut().send_event(VnCommand::LoadedCompatibility(
            rvn_core::LoadCompatibility::LegacyUnchecked,
        ));
        app.update();
        assert_eq!(
            app.world().resource::<Accessibility>().legacy_notice_until,
            20.0
        );
        app.world_mut().send_event(VnCommand::LoadedCompatibility(
            rvn_core::LoadCompatibility::Verified,
        ));
        app.update();
        assert_eq!(
            app.world().resource::<Accessibility>().legacy_notice_until,
            0.0
        );
        drop(app);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn stationary_pointer_and_panel_rebuild_do_not_steal_keyboard_focus() {
        let directory = std::env::temp_dir().join(format!(
            "rvn-hover-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let manager = rvn_core::PersistentDataManager::new(&directory).unwrap();
        let mut app = App::new();
        app.init_resource::<Accessibility>()
            .init_resource::<ButtonInput<MouseButton>>()
            .insert_resource(PersistentDataResource {
                manager,
                data: Default::default(),
            })
            .insert_non_send_resource(Speech::default())
            .add_event::<bevy::input::mouse::MouseWheel>()
            .add_event::<bevy::window::CursorMoved>()
            .add_systems(Update, buttons);
        {
            let mut runtime = app.world_mut().resource_mut::<Accessibility>();
            runtime.open = true;
            runtime.focus = 1;
        }
        app.world_mut()
            .spawn((PanelButton(0), Interaction::Hovered));
        app.update();
        assert_eq!(app.world().resource::<Accessibility>().focus, 1);
        let row = app
            .world_mut()
            .spawn((PanelButton(5), Interaction::Hovered))
            .id();
        app.world_mut().send_event(bevy::window::CursorMoved {
            window: Entity::PLACEHOLDER,
            position: Vec2::new(10.0, 20.0),
            delta: Some(Vec2::new(0.0, 1.0)),
        });
        app.update();
        assert_eq!(app.world().resource::<Accessibility>().focus, 5);
        app.world_mut()
            .get_mut::<Interaction>(row)
            .unwrap()
            .set_if_neq(Interaction::Pressed);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<Accessibility>().focus, 5);
        drop(app);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn assistive_focus_updates_legacy_owners_and_rejects_hidden_controls() {
        use bevy::a11y::{accesskit, ActionRequest};
        let directory = std::env::temp_dir().join(format!(
            "rvn-focus-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let manager = rvn_core::PersistentDataManager::new(&directory).unwrap();
        let mut app = App::new();
        app.init_resource::<Accessibility>()
            .init_resource::<bevy::a11y::Focus>()
            .init_resource::<crate::menu_documents::Menus>()
            .init_resource::<crate::resources::ChoiceFocus>()
            .insert_resource(PersistentDataResource {
                manager,
                data: Default::default(),
            })
            .insert_non_send_resource(Speech::default())
            .add_event::<ActionRequest>()
            .add_systems(Update, assistive_panel);
        let menu = app
            .world_mut()
            .spawn((
                Interaction::None,
                crate::menu_documents::MenuFocus(2, "title/load".into()),
            ))
            .id();
        let choice = app
            .world_mut()
            .spawn((Interaction::None, crate::components::ChoiceButton(3)))
            .id();
        let mut node = accesskit::NodeBuilder::new(accesskit::Role::Button);
        node.set_hidden();
        let hidden = app
            .world_mut()
            .spawn((
                Interaction::None,
                bevy::a11y::AccessibilityNode(node),
                crate::components::ChoiceButton(8),
            ))
            .id();
        fn focus(app: &mut App, entity: Entity) {
            app.world_mut()
                .send_event(ActionRequest(accesskit::ActionRequest {
                    action: accesskit::Action::Focus,
                    target: accesskit::NodeId(entity.to_bits()),
                    data: None,
                }));
            app.update();
        }
        focus(&mut app, menu);
        assert_eq!(
            app.world()
                .resource::<crate::menu_documents::Menus>()
                .keyboard_focus(),
            Some(menu)
        );
        assert_eq!(app.world().resource::<bevy::a11y::Focus>().0, Some(menu));
        focus(&mut app, choice);
        assert_eq!(
            app.world().resource::<crate::resources::ChoiceFocus>().0,
            Some(3)
        );
        assert_eq!(app.world().resource::<bevy::a11y::Focus>().0, Some(choice));
        focus(&mut app, hidden);
        assert_eq!(
            app.world().resource::<crate::resources::ChoiceFocus>().0,
            Some(3)
        );
        assert_eq!(app.world().resource::<bevy::a11y::Focus>().0, Some(choice));
        app.world_mut().resource_mut::<Accessibility>().blocked = true;
        focus(&mut app, menu);
        assert_eq!(app.world().resource::<bevy::a11y::Focus>().0, Some(choice));
        drop(app);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn visual_preferences_are_absolute_reversible_and_do_not_compound() {
        let mut app = App::new();
        app.init_resource::<Accessibility>()
            .add_systems(Update, visuals);
        let color = Color::srgb(0.2, 0.3, 0.4);
        let entity = app
            .world_mut()
            .spawn((
                Node::default(),
                BackgroundColor(color),
                Text::from_section(
                    "Readable",
                    TextStyle {
                        font_size: 20.0,
                        color,
                        ..default()
                    },
                ),
            ))
            .id();
        app.update();
        app.world_mut()
            .resource_mut::<Accessibility>()
            .settings
            .text_scale = 1.5;
        app.world_mut()
            .resource_mut::<Accessibility>()
            .settings
            .high_contrast = true;
        for _ in 0..4 {
            app.update();
            assert_eq!(
                app.world().get::<Text>(entity).unwrap().sections[0]
                    .style
                    .font_size,
                30.0
            );
        }
        assert_eq!(
            app.world().get::<Text>(entity).unwrap().sections[0]
                .style
                .color,
            Color::WHITE
        );
        assert_eq!(
            app.world()
                .get::<BackgroundColor>(entity)
                .unwrap()
                .0
                .to_srgba(),
            Color::BLACK.to_srgba()
        );
        // A typewriter/theme redraw can replace the font size during play.
        app.world_mut().get_mut::<Text>(entity).unwrap().sections[0]
            .style
            .font_size = 24.0;
        app.update();
        assert_eq!(
            app.world().get::<Text>(entity).unwrap().sections[0]
                .style
                .font_size,
            36.0
        );
        app.world_mut().resource_mut::<Accessibility>().settings = Default::default();
        app.update();
        assert_eq!(
            app.world().get::<Text>(entity).unwrap().sections[0]
                .style
                .font_size,
            24.0
        );
        assert_eq!(
            app.world().get::<Text>(entity).unwrap().sections[0]
                .style
                .color,
            color
        );
        assert_eq!(app.world().get::<BackgroundColor>(entity).unwrap().0, color);
    }
    #[test]
    fn keyboard_panel_keeps_focus_blocks_close_key_and_persists_player_overrides() {
        let directory = std::env::temp_dir().join(format!(
            "rvn-accessibility-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let manager = rvn_core::PersistentDataManager::new(&directory).unwrap();
        let mut app = App::new();
        app.init_resource::<Accessibility>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<crate::programmable_ui::Screens>()
            .insert_resource(Settings::default())
            .insert_resource(PersistentDataResource {
                manager: manager.clone(),
                data: Default::default(),
            })
            .insert_non_send_resource(Speech::default())
            .add_systems(Update, keyboard);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F8);
        app.update();
        assert!(app.world().resource::<Accessibility>().open);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowDown);
        app.update();
        assert_eq!(app.world().resource::<Accessibility>().focus, 1);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);
        app.update();
        assert_eq!(
            app.world().resource::<Accessibility>().settings.text_scale,
            1.25
        );
        assert_eq!(
            manager.load().unwrap().accessibility.unwrap().text_scale,
            1.25
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<Accessibility>().open);
        assert!(app.world().resource::<Accessibility>().blocked);
        app.update();
        assert!(!app.world().resource::<Accessibility>().blocked);
        drop(app);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn rich_text_sections_inherit_the_authored_size_instead_of_the_scaled_size() {
        let mut app = App::new();
        app.init_resource::<Accessibility>()
            .add_systems(First, reset_text)
            .add_systems(Update, visuals);
        app.world_mut()
            .resource_mut::<Accessibility>()
            .settings
            .text_scale = 2.0;
        let entity = app
            .world_mut()
            .spawn(Text::from_section(
                "First",
                TextStyle {
                    font_size: 20.0,
                    ..default()
                },
            ))
            .id();
        app.update();
        app.update();
        assert_eq!(
            app.world().get::<Text>(entity).unwrap().sections[0]
                .style
                .font_size,
            40.0
        );
        app.add_systems(
            Update,
            (|mut texts: Query<&mut Text>| {
                for mut text in &mut texts {
                    if text.sections.len() == 1 {
                        let section = text.sections[0].clone();
                        text.sections.push(section);
                    }
                }
            })
            .before(visuals),
        );
        for _ in 0..6 {
            app.update();
        }
        assert!(app
            .world()
            .get::<Text>(entity)
            .unwrap()
            .sections
            .iter()
            .all(|section| section.style.font_size == 40.0));
    }
    #[test]
    fn settled_dialogue_keeps_its_scaled_style_without_a_reset() {
        let mut app = App::new();
        app.init_resource::<crate::resources::TypewriterState>()
            .add_systems(Update, reset_text);
        let entity = app
            .world_mut()
            .spawn((
                Text::from_section(
                    "Reading",
                    TextStyle {
                        font_size: 50.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ),
                OriginalText(vec![(20.0, Color::BLACK, 50.0, Color::WHITE)]),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(entity).unwrap().sections[0]
                .style
                .font_size,
            50.0
        );
        app.world_mut()
            .resource_mut::<crate::resources::TypewriterState>()
            .typing = true;
        app.update();
        assert_eq!(
            app.world().get::<Text>(entity).unwrap().sections[0]
                .style
                .font_size,
            20.0
        );
    }
}
