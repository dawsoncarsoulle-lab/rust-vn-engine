//! Reusable screens share a description with the RVN evaluator and Blueprints.
//! These overlays complement (and never replace) authored game-menu documents.
use crate::{
    resources::{ScriptErrorMessage, VnEngine, VnState},
    vn_command::VnCommand,
};
use bevy::{
    input::{
        keyboard::{Key, KeyboardInput},
        mouse::{MouseScrollUnit, MouseWheel},
        ButtonState,
    },
    prelude::*,
    ui::FocusPolicy,
    window::Ime,
};
use rvn_core::ui::UiInput;
use rvn_parser::Value;
use rvn_ui::programmable::{
    layout_rects, Component as UiComponent, ComponentKind as Kind, ScreenEventKind as EventKind,
    ScreenView, ScrollAxis, TextAlignment,
};
use std::collections::HashMap;

pub struct ProgrammableUiPlugin;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceScrollSet;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceInputSet;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceFocusSet;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceReceiveSet;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceRenderSet;
impl Plugin for ProgrammableUiPlugin {
    fn build(&self, app: &mut App) {
        correct_bevy_ui_antialias(app.world_mut());
        app.init_resource::<Screens>()
            .add_systems(
                PreUpdate,
                transformed_focus
                    .after(bevy::ui::UiSystem::Focus)
                    .in_set(InterfaceFocusSet),
            )
            .add_systems(
                Update,
                (
                    receive
                        .after(crate::systems::stepping_system)
                        .before(input)
                        .in_set(InterfaceReceiveSet),
                    assistive_input.before(input).after(receive),
                    input
                        .before(crate::systems::input_system)
                        .before(crate::systems::player_input_system)
                        .in_set(InterfaceInputSet),
                    render
                        .after(receive)
                        .after(input)
                        .in_set(InterfaceRenderSet),
                    collect_scroll
                        .after(input)
                        .after(crate::custom_canvas::CanvasInputSet)
                        .before(crate::systems::input_system)
                        .before(crate::systems::player_input_system),
                    image_failures.after(render),
                    feedback.after(render),
                ),
            )
            .add_systems(
                PostUpdate,
                scroll
                    .after(bevy::ui::UiSystem::Layout)
                    .after(bevy::transform::TransformSystem::TransformPropagate)
                    .in_set(InterfaceScrollSet),
            );
        #[cfg(target_arch = "wasm32")]
        app.add_systems(PostUpdate, crate::web_inputs::render.after(scroll));
    }
}

// Bevy 0.14.2's UI shader has clamp arguments reversed. WGSL/GLSL defines
// clamp(value,min,max); the old call creates min>max, undefined on WebGL2.
// ANGLE/Firefox consequently paint border colors over the entire node.
// Correct only the known embedded expression, leaving imports/definitions,
// geometry, colors and assets unchanged. Menus use this same shader too.
fn corrected_ui_antialias(source: &str) -> Result<Option<String>, String> {
    const OLD: &str = "clamp(0.0, 1.0, 0.5 - 2.0 * distance)";
    const NEW: &str = "clamp(0.5 - 2.0 * distance, 0.0, 1.0)";
    match source.matches(OLD).count(){
        1=>Ok(Some(source.replacen(OLD,NEW,1))),
        0 if source.matches(NEW).count()==1=>Ok(None),
        _=>Err("Unrecognized Bevy UI antialias shader: verify the dependency before shipping borders and rounded corners".into()),
    }
}
fn correct_bevy_ui_antialias(world: &mut World) {
    use bevy::render::render_resource::Source;
    let Some(mut shaders) = world.get_resource_mut::<Assets<Shader>>() else {
        warn!("Bevy UI shader unavailable: no rendering resources initialized");
        return;
    };
    // The UI shader handle is private in Bevy 0.14. Locate the embedded asset
    // by its source path, not a copied private UUID or an unrelated material.
    let candidates: Vec<_> = shaders
        .iter()
        .filter(|(_, shader)| shader.path.ends_with("ui.wgsl"))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(candidates.len(),1,"Expected the single known Bevy UI shader; initialize DefaultPlugins before ProgrammableUiPlugin");
    let shader = shaders.get_mut(candidates[0]).unwrap();
    let Source::Wgsl(source) = &mut shader.source else {
        panic!("Expected the known WGSL Bevy UI shader")
    };
    if let Some(corrected) = corrected_ui_antialias(source).expect("Unsupported Bevy UI shader") {
        *source = corrected.into();
    }
}

fn image_failures(
    screens: Res<Screens>,
    assets: Res<AssetServer>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
) {
    if !screens.active {
        return;
    }
    fn missing(node: &UiComponent, assets: &AssetServer) -> Option<(String, String)> {
        if !node.visible {
            return None;
        }
        if let Some(path) = &node.image {
            if let Some(handle) = assets.get_handle::<Image>(path.clone()) {
                if let Some(bevy::asset::LoadState::Failed(problem)) =
                    assets.get_load_state(handle.id())
                {
                    return Some((node.id.clone(), format!("{path}: {problem}")));
                }
            }
        }
        if let Some(path) = &node.font {
            if let Some(handle) = assets.get_handle::<Font>(path.clone()) {
                if let Some(bevy::asset::LoadState::Failed(problem)) =
                    assets.get_load_state(handle.id())
                {
                    return Some((node.id.clone(), format!("{path}: {problem}")));
                }
            }
        }
        node.children
            .iter()
            .find_map(|child| missing(child, assets))
    }
    for view in &screens.views {
        if let Some((component, problem)) = missing(&view.root, &assets) {
            error.0 = format!(
                "Screen '{}' / component '{}': interface resource could not be loaded: {}",
                view.name, component, problem
            );
            next.set(VnState::Error);
            break;
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct Screens {
    pub(super) views: Vec<ScreenView>,
    rendered: Vec<ScreenView>,
    size: Vec2,
    layout_text_scale: f32,
    pub(super) active: bool,
    dropdown: Option<Dropdown>,
    rendered_dropdown: Option<Dropdown>,
    editing: Editing,
    rendered_editing: Editing,
    dragging: Option<Dragging>,
    pub(super) keyboard_focus: Option<(String, String)>,
    scroll_positions: HashMap<(String, String), Vec2>,
    reveal_focus: Option<(String, String)>,
    pending_scroll: Vec2,
    modifiers: [bool; 4],
    pub pointer_consumed: bool,
    pub keyboard_consumed: bool,
}
impl Screens {
    pub fn modal(&self) -> bool {
        self.views.iter().any(|view| view.modal)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Editing {
    screen: String,
    element: String,
    caret: usize,
    anchor: Option<usize>,
    composing: bool,
}
#[derive(Clone, Debug, PartialEq)]
struct Dropdown {
    screen: String,
    element: String,
    options: Vec<String>,
    labels: Vec<String>,
    highlighted: usize,
    position: Vec2,
    width: f32,
}
#[derive(Clone)]
struct Dragging {
    screen: String,
    element: String,
}
#[derive(Component)]
struct ScreenRoot;
#[derive(Component)]
struct ScrollPort {
    screen: String,
    element: String,
    axis: ScrollAxis,
    depth: usize,
}
#[derive(Component)]
struct ScrollContent {
    port: Entity,
}
#[derive(Component)]
struct ScrollThumb {
    port: Entity,
    horizontal: bool,
}
#[derive(Component, Clone)]
pub(crate) struct Control {
    pub screen: String,
    pub element: String,
    pub(super) option: Option<String>,
}

fn local_pointer(transform: &GlobalTransform, cursor: Vec2) -> Option<Vec2> {
    let matrix = transform.compute_matrix();
    if !matrix.is_finite() || matrix.determinant().abs() < 1e-8 {
        return None;
    }
    let local = matrix
        .inverse()
        .transform_point3(cursor.extend(transform.translation().z))
        .truncate();
    local.is_finite().then_some(local)
}
fn transformed_contains(size: Vec2, transform: &GlobalTransform, cursor: Vec2) -> bool {
    size.min_element() > 0.0
        && local_pointer(transform, cursor).is_some_and(|point| point.abs().cmple(size * 0.5).all())
}
fn slider_fraction(size: Vec2, transform: &GlobalTransform, cursor: Vec2) -> Option<f32> {
    local_pointer(transform, cursor).map(|point| (point.x / size.x.max(1.0) + 0.5).clamp(0.0, 1.0))
}
/// Bevy 0.14's default UI hit-test ignores scale and rotation. Pick in local
/// coordinates instead, using the actual stack and blocking policies, so
/// animated controls stay clickable and disabled/modal controls absorb hits.
fn transformed_focus(
    screens: Res<Screens>,
    stack: Res<bevy::ui::UiStack>,
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut nodes: Query<(
        &Node,
        &GlobalTransform,
        &ViewVisibility,
        Option<&bevy::ui::CalculatedClip>,
        Option<&FocusPolicy>,
        Option<&Control>,
        Option<&mut Interaction>,
    )>,
) {
    if !screens.active {
        return;
    }
    let cursor = windows
        .get_single()
        .ok()
        .and_then(|window| window.cursor_position());
    let mut picked = None;
    if let Some(cursor) = cursor {
        for entity in stack.uinodes.iter().rev() {
            let Ok((node, transform, visible, clip, policy, control, _)) = nodes.get(*entity)
            else {
                continue;
            };
            if !visible.get()
                || clip.is_some_and(|clip| !clip.clip.contains(cursor))
                || !transformed_contains(node.size(), transform, cursor)
            {
                continue;
            }
            if control.is_some() {
                picked = Some(*entity);
                break;
            }
            if policy == Some(&FocusPolicy::Block) {
                break;
            }
        }
    }
    for entity in &stack.uinodes {
        if let Ok((_, _, _, _, _, Some(_), Some(mut interaction))) = nodes.get_mut(*entity) {
            // Winit may deliver down and up between two frames. Bevy retains
            // both edges even though the final held state is false; preserve
            // that press for this frame so input can dispatch it exactly once.
            let next = if picked != Some(*entity) {
                Interaction::None
            } else if mouse.pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Left) {
                Interaction::Pressed
            } else {
                Interaction::Hovered
            };
            if *interaction != next {
                *interaction = next;
            }
        }
    }
}

fn faded(value: [f32; 4], opacity: f32) -> Color {
    Color::srgba(value[0], value[1], value[2], value[3] * opacity)
}

/// Shared reference layout is resolved once per redraw. Descendant rectangles
/// are translated back to parent-local coordinates for Bevy's hit testing,
/// scrolling and transform animations; all targets retain authored identities.
#[cfg(test)]
fn layout_view(view: &ScreenView) -> ScreenView {
    layout_view_with_text_scale(view, 1.0)
}
fn layout_view_with_text_scale(view: &ScreenView, text_scale: f32) -> ScreenView {
    let mut resolved = view.clone();
    let absolute_root = view.root.rect.is_none()
        && view.root.width.is_none()
        && view.root.height.is_none()
        && !view.root.children.is_empty()
        && view
            .root
            .children
            .iter()
            .filter(|child| child.visible)
            .all(|child| child.rect.is_some());
    if !absolute_root
        && view.root.rect.is_none()
        && matches!(view.root.kind, Kind::Panel | Kind::Row | Kind::Column)
        && view.root.scroll == ScrollAxis::None
    {
        resolved.root.scroll = ScrollAxis::Both;
    }
    let mut measurement = view.root.clone();
    measurement.visit_mut(&mut |component| component.font_size *= text_scale);
    let Ok(rects) = layout_rects(&measurement, [1920.0, 1080.0]) else {
        return resolved;
    };
    let rects: HashMap<_, _> = rects.into_iter().map(|item| (item.id, item.rect)).collect();
    fn apply(node: &mut UiComponent, parent: [f32; 2], rects: &HashMap<String, [f32; 4]>) {
        if let Some(rect) = rects.get(&node.id) {
            node.rect = Some([rect[0] - parent[0], rect[1] - parent[1], rect[2], rect[3]]);
            for child in &mut node.children {
                apply(child, [rect[0], rect[1]], rects);
            }
        }
    }
    apply(&mut resolved.root, [0.0, 0.0], &rects);
    resolved
}
pub(crate) fn update_views(screens: &mut Screens, views: Vec<ScreenView>) {
    // Authored ui.focus(), load and rollback win over transient input focus.
    // Ordinary redraws keep the current screen in the multi-screen tab order.
    if let Some(focus) = views.iter().rev().find_map(|view| {
        let old = screens.views.iter().find(|old| old.name == view.name);
        (old.is_none_or(|old| old.focus != view.focus))
            .then(|| view.focus.clone().map(|id| (view.name.clone(), id)))
            .flatten()
    }) {
        screens.reveal_focus = Some(focus.clone());
        screens.keyboard_focus = Some(focus);
    }
    screens.scroll_positions.retain(|(screen, element), _| {
        views
            .iter()
            .any(|view| &view.name == screen && view.root.find(element).is_some())
    });
    screens.views = views;
}
fn receive(mut commands: EventReader<VnCommand>, mut screens: ResMut<Screens>) {
    for command in commands.read() {
        if let VnCommand::Interfaces(views) = command {
            update_views(&mut screens, views.clone());
        }
    }
    if screens.dropdown.as_ref().is_some_and(|menu| {
        !screens
            .views
            .iter()
            .any(|view| view.name == menu.screen && view.root.find(&menu.element).is_some())
    }) {
        screens.dropdown = None;
    }
}

/// Canvas frame/drawing snapshots do not change the host UI layout. Keep its
/// entities (and focus, DOM fields, scroll state) while only refreshing canvas
/// leaves. Any authored layout/style/control change still follows full redraw.
fn same_host_views(left: &[ScreenView], right: &[ScreenView]) -> bool {
    fn host(views: &[ScreenView]) -> Vec<ScreenView> {
        let mut views = views.to_vec();
        for view in &mut views {
            view.root.visit_mut(&mut |component| {
                if component.kind == Kind::Canvas {
                    component.drawing = None;
                    if let Some(frame) = &mut component.canvas_frame {
                        frame.time = 0.0;
                    }
                }
            });
        }
        views
    }
    left == right || host(left) == host(right)
}

fn render(
    mut commands: Commands,
    mut screens: ResMut<Screens>,
    roots: Query<Entity, With<ScreenRoot>>,
    windows: Query<&Window>,
    assets: Res<AssetServer>,
    font: Res<crate::menu_documents::MenuFont>,
    state: Res<State<VnState>>,
    accessibility: Res<crate::accessibility::Accessibility>,
    source: Option<Res<crate::source_menus::SourceMenus>>,
) {
    let active = !screens.views.is_empty()
        && (source.as_deref().is_some_and(|source|source.any()) || matches!(
            state.get(),
            VnState::Waiting | VnState::Stepping | VnState::Animating
        ));
    let Ok(window) = windows.get_single() else {
        return;
    };
    let size = Vec2::new(window.width(), window.height());
    if screens.rendered == screens.views
        && screens.size == size
        && screens.active == active
        && screens.rendered_dropdown == screens.dropdown
        && screens.rendered_editing == screens.editing
        && screens.layout_text_scale == accessibility.settings.text_scale
    {
        return;
    }
    if screens.size == size
        && screens.active == active
        && screens.rendered_dropdown == screens.dropdown
        && screens.rendered_editing == screens.editing
        && screens.layout_text_scale == accessibility.settings.text_scale
        && same_host_views(&screens.rendered, &screens.views)
    {
        screens.rendered = screens.views.clone();
        return;
    }
    debug!(
        "Interface redraw: views={}, size={}, active={}, dropdown={}, editing={}",
        screens.rendered != screens.views,
        screens.size != size,
        screens.active != active,
        screens.rendered_dropdown != screens.dropdown,
        screens.rendered_editing != screens.editing
    );
    for root in &roots {
        commands.entity(root).despawn_recursive();
    }
    screens.rendered = screens.views.clone();
    screens.size = size;
    screens.active = active;
    screens.layout_text_scale = accessibility.settings.text_scale;
    screens.rendered_dropdown = screens.dropdown.clone();
    screens.rendered_editing = screens.editing.clone();
    if !active {
        return;
    }
    let modal = screens.views.iter().rposition(|view| view.modal);
    for (index, view) in screens.views.iter().enumerate() {
        let interactive = modal.is_none_or(|modal| index >= modal);
        let root = commands
            .spawn((
                ScreenRoot,
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    background_color: if view.modal {
                        Color::srgba(0.0, 0.0, 0.0, 0.45).into()
                    } else {
                        Color::NONE.into()
                    },
                    focus_policy: if view.modal {
                        FocusPolicy::Block
                    } else {
                        FocusPolicy::Pass
                    },
                    z_index: ZIndex::Global(3000 + index as i32 * 10),
                    ..default()
                },
            ))
            .id();
        let resolved = layout_view_with_text_scale(view, accessibility.settings.text_scale);
        spawn(
            &mut commands,
            root,
            &resolved,
            &resolved.root,
            interactive,
            &assets,
            &font.0,
            size,
            &screens.editing,
            &screens.scroll_positions,
            1.0,
            0.0,
            0,
        );
    }
    if let Some(menu) = &screens.dropdown {
        let visible = ((size.y - 24.0) / 36.0).floor().clamp(1.0, 8.0) as usize;
        let first = dropdown_first(menu, visible);
        let height = menu.options.len().min(visible) as f32 * 36.0 + 8.0;
        let width = menu.width.max(180.0).min((size.x - 24.0).max(40.0));
        let top = if menu.position.y + height > size.y - 12.0 {
            (menu.position.y - height - 44.0).max(12.0)
        } else {
            menu.position.y.max(12.0)
        };
        let root = commands
            .spawn((
                ScreenRoot,
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        left: Val::Px(
                            menu.position
                                .x
                                .clamp(12.0, (size.x - width - 12.0).max(12.0)),
                        ),
                        top: Val::Px(top),
                        width: Val::Px(width),
                        height: Val::Px(height),
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::clip_y(),
                        padding: UiRect::all(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    background_color: Color::srgb(0.04, 0.07, 0.10).into(),
                    border_color: Color::srgb(0.2, 0.5, 0.7).into(),
                    z_index: ZIndex::Global(5000),
                    ..default()
                },
            ))
            .id();
        for (index, option) in menu.options.iter().enumerate().skip(first).take(visible) {
            let button = commands
                .spawn((
                    Control {
                        screen: menu.screen.clone(),
                        element: menu.element.clone(),
                        option: Some(option.clone()),
                    },
                    ButtonBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Px(36.0),
                            flex_shrink: 0.0,
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        background_color: if index == menu.highlighted {
                            Color::srgb(0.07, 0.28, 0.42).into()
                        } else {
                            Color::NONE.into()
                        },
                        ..default()
                    },
                ))
                .id();
            commands.entity(root).add_child(button);
            let text = commands
                .spawn(TextBundle::from_section(
                    menu.labels.get(index).unwrap_or(option),
                    TextStyle {
                        font: font.0.clone(),
                        font_size: 20.0,
                        color: Color::WHITE,
                    },
                ))
                .id();
            commands.entity(button).add_child(text);
        }
    }
}

fn dropdown_first(menu: &Dropdown, visible: usize) -> usize {
    menu.highlighted
        .saturating_sub(visible.saturating_sub(1))
        .min(menu.options.len().saturating_sub(visible))
}

/// Authoring controls typography; only keep a one-physical-pixel safety floor.
/// Browser input overlays use the same base size before accessibility scaling.
pub(super) fn component_font_size(font_size: f32, scale: f32) -> f32 {
    (font_size * scale).max(1.0)
}

fn spawn(
    commands: &mut Commands,
    parent: Entity,
    view: &ScreenView,
    node: &UiComponent,
    ancestors_enabled: bool,
    assets: &AssetServer,
    font: &Handle<Font>,
    size: Vec2,
    editing: &Editing,
    positions: &HashMap<(String, String), Vec2>,
    inherited_opacity: f32,
    parent_border: f32,
    depth: usize,
) {
    if !node.visible {
        return;
    }
    let enabled = ancestors_enabled && node.enabled;
    let opacity = inherited_opacity * node.opacity;
    let selected_font = node
        .font
        .as_ref()
        .map(|path| assets.load::<Font>(path.clone()));
    let font = selected_font.as_ref().unwrap_or(font);
    let control = node.is_control() || matches!(node.kind, Kind::Button | Kind::Canvas);
    let scale = (size.x / 1920.0).min(size.y / 1080.0).max(0.25);
    let border_width = node.border_width.unwrap_or(if control { 1.0 } else { 0.0 }) * scale;
    let mut style = Style {
        width: node.width.map(|v| Val::Px(v * scale)).unwrap_or(Val::Auto),
        height: node.height.map(|v| Val::Px(v * scale)).unwrap_or(Val::Auto),
        min_width: node
            .min_width
            .map(|value| Val::Px(value * scale))
            .unwrap_or(if control {
                Val::Px(180.0 * scale)
            } else {
                Val::Auto
            }),
        min_height: node
            .min_height
            .map(|value| Val::Px(value * scale))
            .unwrap_or(if control {
                Val::Px(44.0 * scale)
            } else {
                Val::Auto
            }),
        max_width: node
            .max_width
            .map(|value| Val::Px(value * scale))
            .unwrap_or(Val::Auto),
        max_height: node
            .max_height
            .map(|value| Val::Px(value * scale))
            .unwrap_or(Val::Auto),
        flex_shrink: 0.0,
        flex_direction: if matches!(node.kind, Kind::Row | Kind::Toggle) {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        align_items: if node.kind == Kind::Toggle {
            AlignItems::Center
        } else {
            AlignItems::Stretch
        },
        row_gap: Val::Px(node.spacing * scale),
        column_gap: Val::Px(node.spacing * scale),
        padding: UiRect::all(Val::Px(if control && node.padding == 0.0 {
            10.0 * scale
        } else {
            node.padding * scale
        })),
        border: UiRect::all(Val::Px(border_width)),
        justify_content: if control {
            JustifyContent::Center
        } else {
            JustifyContent::FlexStart
        },
        ..default()
    };
    if let Some(rect) = node.rect {
        // Taffy resolves absolute children from the parent's inner border.
        // RVN reference rectangles use its outer top-left, as in the designer.
        style.position_type = PositionType::Absolute;
        style.left = Val::Px(rect[0] * size.x / 1920.0 - parent_border);
        style.top = Val::Px(rect[1] * size.y / 1080.0 - parent_border);
        style.width = Val::Px(rect[2] * size.x / 1920.0);
        style.height = Val::Px(rect[3] * size.y / 1080.0);
        // The common layout already accounts for padding and margins.
        style.padding = UiRect::ZERO;
        style.min_width = Val::Auto;
        style.min_height = Val::Auto;
        style.max_width = Val::Auto;
        style.max_height = Val::Auto;
    }
    if node.clip {
        style.overflow = Overflow::clip();
    }
    // Absolute children do not contribute to a flex parent's measured size.
    // Give an implicit absolute-layout root the screen's extent; otherwise
    // auto-scroll would clip all of its children to an empty rectangle.
    let absolute_root = depth == 0
        && node.rect.is_none()
        && node.width.is_none()
        && node.height.is_none()
        && !node.children.is_empty()
        && node
            .children
            .iter()
            .filter(|child| child.visible)
            .all(|child| child.rect.is_some());
    if absolute_root {
        style.width = Val::Px(size.x);
        style.height = Val::Px(size.y);
    }
    // A top-level flow layout must stay reachable on a small window, even
    // when its author hasn't explicitly added a scroll container.
    let axis = node.scroll;
    let content_style = style.clone();
    if axis != ScrollAxis::None {
        style.overflow = Overflow::clip();
        style.max_width = Val::Px((size.x - 32.0).max(40.0));
        style.max_height = Val::Px((size.y - 32.0).max(40.0));
        style.padding = UiRect::ZERO;
        style.row_gap = Val::Px(0.0);
        style.column_gap = Val::Px(0.0);
    }
    let background = if control
        && node.kind != Kind::Canvas
        && node.auto_background
        && node.background[3] == 0.0
    {
        faded([0.06, 0.12, 0.18, 1.0], opacity)
    } else {
        faded(node.background, opacity)
    };
    let entity = commands
        .spawn((
            crate::composed_motion::InterfaceTarget {
                screen: view.name.clone(),
                element: node.id.clone(),
            },
            NodeBundle {
                style,
                background_color: background.into(),
                border_color: faded(node.border_color, opacity).into(),
                focus_policy: FocusPolicy::Pass,
                border_radius: BorderRadius::all(Val::Px(node.radius * scale)),
                ..default()
            },
        ))
        .id();
    commands.entity(parent).add_child(entity);
    let children_parent = if axis != ScrollAxis::None {
        commands.entity(entity).insert(ScrollPort {
            screen: view.name.clone(),
            element: node.id.clone(),
            axis,
            depth,
        });
        let offset = positions
            .get(&(view.name.clone(), node.id.clone()))
            .copied()
            .unwrap_or_default();
        let content_width = node
            .children
            .iter()
            .filter_map(|child| child.rect)
            .map(|rect| rect[0] + rect[2])
            .fold(node.rect.map_or(0.0, |rect| rect[2]), f32::max);
        let content_height = node
            .children
            .iter()
            .filter_map(|child| child.rect)
            .map(|rect| rect[1] + rect[3])
            .fold(node.rect.map_or(0.0, |rect| rect[3]), f32::max);
        let content = commands
            .spawn((
                ScrollContent { port: entity },
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Relative,
                        left: Val::Px(-offset.x - border_width),
                        top: Val::Px(-offset.y - border_width),
                        width: Val::Px(content_width * size.x / 1920.0),
                        height: Val::Px(content_height * size.y / 1080.0),
                        flex_shrink: 0.0,
                        flex_direction: content_style.flex_direction,
                        row_gap: content_style.row_gap,
                        column_gap: content_style.column_gap,
                        padding: content_style.padding,
                        ..default()
                    },
                    focus_policy: FocusPolicy::Pass,
                    ..default()
                },
            ))
            .id();
        commands.entity(entity).add_child(content);
        content
    } else {
        entity
    };
    if axis != ScrollAxis::None {
        for horizontal in [false, true] {
            if if horizontal {
                axis.horizontal()
            } else {
                axis.vertical()
            } {
                let thumb = commands
                    .spawn((
                        ScrollThumb {
                            port: entity,
                            horizontal,
                        },
                        NodeBundle {
                            style: Style {
                                position_type: PositionType::Absolute,
                                width: Val::Px(4.0),
                                height: Val::Px(4.0),
                                ..default()
                            },
                            background_color: faded([0.19, 0.40, 0.52, 1.0], opacity).into(),
                            border_radius: BorderRadius::all(Val::Px(2.0)),
                            visibility: Visibility::Hidden,
                            focus_policy: FocusPolicy::Pass,
                            z_index: ZIndex::Local(1),
                            ..default()
                        },
                    ))
                    .id();
                commands.entity(entity).add_child(thumb);
            }
        }
    }
    // Disabled controls still absorb the pointer: clicking them must never
    // click through into the story or another screen beneath them.
    if control {
        commands.entity(entity).insert((
            Interaction::None,
            FocusPolicy::Block,
            Control {
                screen: view.name.clone(),
                element: node.id.clone(),
                option: None,
            },
        ));
    }
    if node.kind == Kind::Canvas {
        if let Some(drawing) = &node.drawing {
            let extent = node
                .canvas_frame
                .map(|frame| Vec2::new(frame.width, frame.height))
                .or_else(|| node.rect.map(|rect| Vec2::new(rect[2], rect[3])))
                .unwrap_or(Vec2::new(
                    node.width.unwrap_or(320.0),
                    node.height.unwrap_or(240.0),
                ));
            commands
                .entity(entity)
                .insert(crate::custom_canvas::CanvasSurface {
                    screen: view.name.clone(),
                    element: node.id.clone(),
                    drawing: drawing.clone(),
                    extent,
                    font: font.clone(),
                    opacity,
                });
        }
    }
    if node.kind == Kind::Toggle {
        let checked = node.value.as_bool().unwrap_or(false);
        let box_node = commands
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Px(18.0 * scale),
                    height: Val::Px(18.0 * scale),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                background_color: faded(
                    if checked {
                        [0.08, 0.57, 0.76, 1.0]
                    } else {
                        [0.03, 0.06, 0.09, 1.0]
                    },
                    opacity,
                )
                .into(),
                border_color: faded([0.30, 0.55, 0.68, 1.0], opacity).into(),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                focus_policy: FocusPolicy::Pass,
                ..default()
            })
            .id();
        commands.entity(entity).add_child(box_node);
        if checked {
            let mark = commands
                .spawn(TextBundle::from_section(
                    "✓",
                    TextStyle {
                        font: font.clone(),
                        font_size: (16.0 * scale).max(12.0),
                        color: faded([1.0; 4], opacity),
                    },
                ))
                .id();
            commands.entity(box_node).add_child(mark);
        }
    }
    if node.kind == Kind::Image {
        if let Some(label) = node
            .accessible_label
            .as_ref()
            .filter(|label| !label.is_empty())
        {
            let mut semantics =
                bevy::a11y::accesskit::NodeBuilder::new(bevy::a11y::accesskit::Role::Image);
            semantics.set_name(label.clone());
            if !enabled {
                semantics.set_disabled();
            }
            commands
                .entity(entity)
                .insert(bevy::a11y::AccessibilityNode::from(semantics));
        }
        let image = commands
            .spawn(ImageBundle {
                image: UiImage::new(assets.load(node.image.clone().unwrap()))
                    .with_color(Color::srgba(1.0, 1.0, 1.0, opacity)),
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                focus_policy: FocusPolicy::Pass,
                ..default()
            })
            .id();
        commands.entity(entity).add_child(image);
    } else if !matches!(
        node.kind,
        Kind::Panel | Kind::Column | Kind::Row | Kind::Canvas
    ) {
        let mut label = match node.kind {
            Kind::Input => {
                if cfg!(target_arch = "wasm32") && enabled {
                    String::new()
                } else {
                    node.value.as_str().unwrap_or_default().to_owned()
                }
            }
            Kind::Select => format!(
                "{}  ▾",
                node.option_label(node.value.as_str().unwrap_or_default())
            ),
            Kind::Toggle => node.text.clone(),
            Kind::Slider => format!(
                "{}  {:.0} %",
                node.text,
                (node.value.as_f64().unwrap_or(node.min) - node.min) / (node.max - node.min)
                    * 100.0
            ),
            _ => node.text.clone(),
        };
        if node.kind == Kind::Input && !cfg!(target_arch = "wasm32") {
            if editing.screen == view.name && editing.element == node.id {
                let caret = editing.caret.min(label.len());
                if label.is_char_boundary(caret) {
                    label.insert(caret, '│');
                }
            } else if label.is_empty() {
                label = node.placeholder.clone();
            }
        }
        let justify = match node.text_align {
            TextAlignment::Left => JustifyText::Left,
            TextAlignment::Center => JustifyText::Center,
            TextAlignment::Right => JustifyText::Right,
        };
        let mut bundle = TextBundle::from_section(
            label,
            TextStyle {
                font: font.clone(),
                font_size: component_font_size(node.font_size, scale),
                color: if enabled {
                    faded(node.foreground, opacity)
                } else {
                    faded([0.4, 0.45, 0.5, 1.0], opacity)
                },
            },
        )
        .with_text_justify(justify);
        bundle.style.width = Val::Percent(100.0);
        bundle.style.padding = UiRect::all(Val::Px(if control && node.padding == 0.0 {
            10.0 * scale
        } else {
            node.padding * scale
        }));
        let text = commands.spawn(bundle).id();
        commands.entity(entity).add_child(text);
    }
    if node.kind == Kind::Slider {
        let fraction = ((node.value.as_f64().unwrap_or(node.min) - node.min)
            / (node.max - node.min))
            .clamp(0.0, 1.0) as f32;
        let track = commands
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Px(6.0),
                    margin: UiRect::top(Val::Px(3.0)),
                    ..default()
                },
                background_color: faded([0.02, 0.06, 0.09, 1.0], opacity).into(),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                focus_policy: FocusPolicy::Pass,
                ..default()
            })
            .id();
        commands.entity(entity).add_child(track);
        let fill = commands
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(fraction * 100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                background_color: faded([0.08, 0.58, 0.78, 1.0], opacity).into(),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                focus_policy: FocusPolicy::Pass,
                ..default()
            })
            .id();
        commands.entity(track).add_child(fill);
        let handle = commands
            .spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(fraction * 100.0),
                    top: Val::Px(-3.0),
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    margin: UiRect::left(Val::Px(-6.0)),
                    ..default()
                },
                background_color: faded([0.75, 0.93, 1.0, 1.0], opacity).into(),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                focus_policy: FocusPolicy::Pass,
                ..default()
            })
            .id();
        commands.entity(track).add_child(handle);
    }
    for child in &node.children {
        spawn(
            commands,
            children_parent,
            view,
            child,
            enabled,
            assets,
            font,
            size,
            editing,
            positions,
            opacity,
            if axis == ScrollAxis::None {
                border_width
            } else {
                0.0
            },
            depth + 1,
        );
    }
}

fn collect_scroll(
    mut screens: ResMut<Screens>,
    windows: Query<&Window>,
    ports: Query<(&ScrollPort, &Node, &GlobalTransform)>,
    mut wheel: EventReader<MouseWheel>,
) {
    let movement = wheel.read().fold(Vec2::ZERO, |sum, event| {
        sum + Vec2::new(event.x, event.y)
            * if event.unit == MouseScrollUnit::Line {
                36.0
            } else {
                1.0
            }
    });
    if !screens.active
        || screens.dropdown.is_some()
        || screens.pointer_consumed
        || movement == Vec2::ZERO
    {
        return;
    }
    let cursor = windows
        .get_single()
        .ok()
        .and_then(|window| window.cursor_position());
    let modal = screens
        .views
        .iter()
        .rposition(|view| view.modal)
        .unwrap_or(0);
    if ports.iter().any(|(port, node, transform)| {
        screens
            .views
            .iter()
            .skip(modal)
            .any(|view| view.name == port.screen)
            && cursor.is_some_and(|cursor| transformed_contains(node.size(), transform, cursor))
    }) {
        screens.pointer_consumed = true;
        screens.pending_scroll += movement;
    }
}

fn scroll(
    mut screens: ResMut<Screens>,
    windows: Query<&Window>,
    ports: Query<(Entity, &ScrollPort, &Node, &GlobalTransform)>,
    mut contents: Query<(&ScrollContent, &Node, &mut Style), Without<ScrollThumb>>,
    mut thumbs: Query<(&ScrollThumb, &mut Style, &mut Visibility), Without<ScrollContent>>,
    controls: Query<(Entity, &Control, &Node, &GlobalTransform)>,
    parents: Query<&Parent>,
) {
    // Geometry is only authoritative after layout and transform propagation.
    // Reading it during Update can acknowledge a focus request against an old
    // or newly spawned layout, leaving the requested control off screen.
    let movement = std::mem::take(&mut screens.pending_scroll);
    if !screens.active || screens.dropdown.is_some() {
        return;
    }
    let cursor = windows
        .get_single()
        .ok()
        .and_then(|window| window.cursor_position());
    let modal = screens
        .views
        .iter()
        .rposition(|view| view.modal)
        .unwrap_or(0);
    let hit = ports
        .iter()
        .filter(|(_, port, node, transform)| {
            screens
                .views
                .iter()
                .skip(modal)
                .any(|view| view.name == port.screen)
                && cursor.is_some_and(|cursor| {
                    (cursor - transform.translation().truncate())
                        .abs()
                        .cmple(node.size() * 0.5)
                        .all()
                })
        })
        .max_by_key(|(_, port, _, _)| {
            (
                screens
                    .views
                    .iter()
                    .position(|view| view.name == port.screen)
                    .unwrap_or(0),
                port.depth,
            )
        })
        .map(|(entity, _, _, _)| entity);
    let reveal = screens.reveal_focus.clone();
    let focused = reveal.as_ref().and_then(|(screen, element)| {
        controls.iter().find(|(_, control, node, _)| {
            &control.screen == screen
                && &control.element == element
                && control.option.is_none()
                && node.size().cmpgt(Vec2::ZERO).all()
        })
    });
    for (content, node, mut style) in &mut contents {
        let Ok((entity, port, viewport, transform)) = ports.get(content.port) else {
            continue;
        };
        // Commands spawn fresh nodes before Bevy's layout pass. Their zero
        // dimensions must not erase scroll restoration or pending focus.
        if !viewport.size().cmpgt(Vec2::ZERO).all() || !node.size().cmpgt(Vec2::ZERO).all() {
            continue;
        }
        let max = (node.size() - viewport.size()).max(Vec2::ZERO);
        let key = (port.screen.clone(), port.element.clone());
        let mut offset = screens
            .scroll_positions
            .get(&key)
            .copied()
            .unwrap_or_default();
        if hit == Some(entity) && movement != Vec2::ZERO && max != Vec2::ZERO {
            if port.axis.vertical() {
                offset.y -= movement.y;
            }
            if port.axis.horizontal() {
                offset.x -= if movement.x != 0.0 {
                    movement.x
                } else if !port.axis.vertical() {
                    movement.y
                } else {
                    0.0
                };
            }
            screens.pointer_consumed = true;
        }
        if let Some((focused_entity, control, focused_node, focused_transform)) = focused {
            // Only ancestors of this exact entity may reveal it.
            let mut ancestor = focused_entity;
            let mut owned = false;
            while let Ok(parent) = parents.get(ancestor) {
                ancestor = parent.get();
                if ancestor == entity {
                    owned = true;
                    break;
                }
            }
            if owned && port.screen == control.screen {
                let center =
                    focused_transform.translation().truncate() - transform.translation().truncate();
                let low = center - focused_node.size() * 0.5;
                let high = center + focused_node.size() * 0.5;
                debug!("Reveal {} / {}: viewport={:?}, content={:?}, center={center:?}, offset={offset:?}",port.screen,control.element,viewport.size(),node.size());
                if port.axis.vertical() {
                    if low.y < -viewport.size().y * 0.5 {
                        offset.y += low.y + viewport.size().y * 0.5;
                    } else if high.y > viewport.size().y * 0.5 {
                        offset.y += high.y - viewport.size().y * 0.5;
                    }
                }
                if port.axis.horizontal() {
                    if low.x < -viewport.size().x * 0.5 {
                        offset.x += low.x + viewport.size().x * 0.5;
                    } else if high.x > viewport.size().x * 0.5 {
                        offset.x += high.x - viewport.size().x * 0.5;
                    }
                }
            }
        }
        if !port.axis.vertical() {
            offset.y = 0.0;
        }
        if !port.axis.horizontal() {
            offset.x = 0.0;
        }
        offset = offset.clamp(Vec2::ZERO, max);
        if style.left != Val::Px(-offset.x) {
            style.left = Val::Px(-offset.x);
        }
        if style.top != Val::Px(-offset.y) {
            style.top = Val::Px(-offset.y);
        }
        for (thumb, mut style, mut visibility) in &mut thumbs {
            if thumb.port != entity {
                continue;
            }
            let (viewport_length, content_length, position, maximum) = if thumb.horizontal {
                (viewport.size().x, node.size().x, offset.x, max.x)
            } else {
                (viewport.size().y, node.size().y, offset.y, max.y)
            };
            *visibility = if maximum > 0.5 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            let track = (viewport_length - 12.0).max(1.0);
            let length =
                (track * viewport_length / content_length.max(1.0)).clamp(16.0, track.max(16.0));
            let start = 6.0 + (track - length).max(0.0) * position / maximum.max(1.0);
            if thumb.horizontal {
                style.width = Val::Px(length);
                style.height = Val::Px(4.0);
                style.left = Val::Px(start);
                style.bottom = Val::Px(3.0);
            } else {
                style.height = Val::Px(length);
                style.width = Val::Px(4.0);
                style.top = Val::Px(start);
                style.right = Val::Px(3.0);
            }
        }
        screens.scroll_positions.insert(key, offset);
    }
    if focused.is_some() {
        screens.reveal_focus = None;
    }
}

fn feedback(
    screens: Res<Screens>,
    mut controls: Query<(
        &Control,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    for (control, interaction, mut background, mut border) in &mut controls {
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
        let enabled = view.root.available(&control.element)
            && !screens
                .views
                .iter()
                .any(|other| other.modal && (other.layer, other.order) > (view.layer, view.order));
        let focused = enabled
            && screens.keyboard_focus.as_ref().map_or(
                view.focus.as_ref() == Some(&control.element),
                |(screen, element)| screen == &control.screen && element == &control.element,
            );
        let opacity = view.root.effective_opacity(&control.element).unwrap_or(1.0);
        *border = faded(
            if focused {
                component.focus_color.unwrap_or([0.12, 0.72, 1.0, 1.0])
            } else {
                component.border_color
            },
            opacity,
        )
        .into();
        *background = match interaction {
            _ if component.kind == Kind::Canvas => faded(component.background, opacity).into(),
            _ if !enabled => faded(component.background, opacity).into(),
            Interaction::Pressed => faded(
                component
                    .pressed_background
                    .unwrap_or([0.05, 0.37, 0.52, 1.0]),
                opacity,
            )
            .into(),
            Interaction::Hovered => faded(
                component
                    .hover_background
                    .unwrap_or([0.10, 0.25, 0.34, 1.0]),
                opacity,
            )
            .into(),
            _ if control.option.as_ref().is_some_and(|option| {
                screens
                    .dropdown
                    .as_ref()
                    .is_some_and(|menu| menu.options.get(menu.highlighted) == Some(option))
            }) =>
            {
                Color::srgb(0.07, 0.28, 0.42).into()
            }
            _ if control.option.is_none()
                && (component.background[3] != 0.0 || !component.auto_background) =>
            {
                faded(component.background, opacity).into()
            }
            _ => faded([0.06, 0.12, 0.18, 1.0], opacity).into(),
        };
    }
}

fn send(
    engine: &mut VnEngine,
    event: UiInput,
    screens: &mut Screens,
    output: &mut EventWriter<VnCommand>,
    error: &mut ScriptErrorMessage,
    next: &mut NextState<VnState>,
) -> bool {
    if engine.0.renderer.menu_pending {return false;}
    let source_event=engine.0.state.ui.screens.iter().any(|screen|screen.name==event.screen && screen.host_role.is_some());
    if let Err(problem) = engine.0.interface_event(event) {
        error.0 = rvn_core::error::ScriptError::from_runtime(&problem).to_string();
        if source_event||problem.is_menu_request_rejection() {warn!("Game menu event: {}",error.0);} else {next.set(VnState::Error);}
        return false;
    }
    match engine.0.interface_views() {
        Ok(views) => update_views(screens, views),
        Err(problem) => {
            error.0 = rvn_core::error::ScriptError::from_runtime(&problem).to_string();
            next.set(VnState::Error);
            return false;
        }
    }
    for command in engine.0.renderer.take_pending() {
        output.send(command);
    }
    true
}

fn event(screen: &str, element: &str, kind: EventKind, value: Option<Value>) -> UiInput {
    UiInput {
        screen: screen.into(),
        element: element.into(),
        kind,
        value,
        key: None,
    }
}

/// Assistive technology uses the same validated event path as pointer and
/// keyboard input. Stale, hidden, disabled and covered controls are ignored.
fn assistive_input(
    mut requests: EventReader<bevy::a11y::ActionRequest>,
    mut screens: ResMut<Screens>,
    mut engine: ResMut<VnEngine>,
    controls: Query<(&Control, &Node, &GlobalTransform)>,
    mut output: EventWriter<VnCommand>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
    state: Res<State<VnState>>,
    accessibility: Res<crate::accessibility::Accessibility>,
    source: Option<Res<crate::source_menus::SourceMenus>>,
) {
    use bevy::a11y::accesskit::Action;
    for request in requests.read() {
        if accessibility.blocked
            || (!source.as_deref().is_some_and(|source|source.any()) && !matches!(
                state.get(),
                VnState::Waiting | VnState::Stepping | VnState::Animating
            ))
        {
            continue;
        }
        let Ok(entity) = Entity::try_from_bits(request.0.target.0) else {
            continue;
        };
        let Ok((control, node, transform)) = controls.get(entity) else {
            continue;
        };
        let Some(view) = screens
            .views
            .iter()
            .find(|view| view.name == control.screen)
            .cloned()
        else {
            continue;
        };
        let Some(component) = view.root.find(&control.element).cloned() else {
            continue;
        };
        if !view.root.available(&control.element)
            || screens
                .views
                .iter()
                .any(|other| other.modal && (other.layer, other.order) > (view.layer, view.order))
        {
            continue;
        }
        if request.0.action == Action::Focus {
            if !send(
                &mut engine,
                event(&control.screen, &control.element, EventKind::Focus, None),
                &mut screens,
                &mut output,
                &mut error,
                &mut next,
            ) {
                return;
            }
            screens.keyboard_focus = Some((control.screen.clone(), control.element.clone()));
            screens.reveal_focus = screens.keyboard_focus.clone();
            continue;
        }
        if request.0.action == Action::Default
            && component.kind == Kind::Select
            && control.option.is_none()
        {
            screens.dropdown = Some(Dropdown {
                screen: control.screen.clone(),
                element: control.element.clone(),
                options: component.options.clone(),
                labels: component.option_labels.clone(),
                highlighted: component
                    .options
                    .iter()
                    .position(|value| component.value.as_str() == Some(value))
                    .unwrap_or(0),
                position: transform.translation().truncate()
                    + Vec2::new(-node.size().x * 0.5, node.size().y * 0.5),
                width: node.size().x,
            });
            continue;
        }
        let input = if let Some(option) = &control.option {
            (request.0.action == Action::Default).then(|| {
                event(
                    &control.screen,
                    &control.element,
                    EventKind::Change,
                    Some(Value::Str(option.clone())),
                )
            })
        } else {
            assistive_event(&control.screen, &component, &request.0)
        };
        if let Some(input) = input {
            screens.dropdown = None;
            if !send(
                &mut engine,
                input,
                &mut screens,
                &mut output,
                &mut error,
                &mut next,
            ) {
                return;
            }
        }
    }
}
fn assistive_event(
    screen: &str,
    component: &UiComponent,
    request: &bevy::a11y::accesskit::ActionRequest,
) -> Option<UiInput> {
    use bevy::a11y::accesskit::{Action, ActionData};
    let value = match (&component.kind, request.action, &request.data) {
        (Kind::Button, Action::Default, _) => {
            return Some(event(screen, &component.id, EventKind::Activate, None))
        }
        (Kind::Canvas, Action::Default, _)
            if component.events.contains_key(&EventKind::Activate) =>
        {
            return Some(event(screen, &component.id, EventKind::Activate, None))
        }
        (Kind::Toggle, Action::Default, _) => {
            Value::Bool(!component.value.as_bool().unwrap_or(false))
        }
        (Kind::Input, Action::SetValue, Some(ActionData::Value(value)))
            if value.len() <= 65_536 && !value.contains('\0') =>
        {
            Value::Str(value.to_string())
        }
        (Kind::Select, Action::SetValue, Some(ActionData::Value(value)))
            if component
                .options
                .iter()
                .any(|option| option == value.as_ref()) =>
        {
            Value::Str(value.to_string())
        }
        (Kind::Slider, Action::SetValue, Some(ActionData::NumericValue(value)))
            if value.is_finite() =>
        {
            Value::Float(value.clamp(component.min, component.max) as f32)
        }
        (Kind::Slider, Action::SetValue, Some(ActionData::Value(value))) => {
            let value = value.parse::<f64>().ok()?;
            if !value.is_finite() {
                return None;
            }
            Value::Float(value.clamp(component.min, component.max) as f32)
        }
        _ => return None,
    };
    Some(event(screen, &component.id, EventKind::Change, Some(value)))
}

fn input(
    mut screens: ResMut<Screens>,
    mut engine: ResMut<VnEngine>,
    controls: Query<(&Control, &Interaction, &Node, &GlobalTransform)>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window>,
    mut keyboard: EventReader<KeyboardInput>,
    mut ime: EventReader<Ime>,
    mut wheel: EventReader<MouseWheel>,
    mut output: EventWriter<VnCommand>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
    state: Res<State<VnState>>,
    accessibility: Res<crate::accessibility::Accessibility>,
    source: Option<Res<crate::source_menus::SourceMenus>>,
) {
    if accessibility.blocked {
        keyboard.clear();
        ime.clear();
        wheel.clear();
        screens.pointer_consumed = true;
        screens.keyboard_consumed = true;
        return;
    }
    screens.pointer_consumed = false;
    screens.keyboard_consumed = false;
    // Drain even while menus/DOM fields own focus; otherwise a release could
    // be lost and leave Shift/Ctrl stuck on the next canvas interaction.
    let raw_keys: Vec<_> = keyboard.read().cloned().collect();
    let has_control = raw_keys
        .iter()
        .any(|key| matches!(key.key_code, KeyCode::ControlLeft | KeyCode::ControlRight));
    let has_shift = raw_keys
        .iter()
        .any(|key| matches!(key.key_code, KeyCode::ShiftLeft | KeyCode::ShiftRight));
    let mut pressed = key_chords(raw_keys.into_iter(), &mut screens.modifiers);
    for (_, control, shift) in &mut pressed {
        if !has_control {
            *control |= keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        }
        if !has_shift {
            *shift |= keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        }
    }
    screens.modifiers = [
        keys.pressed(KeyCode::ControlLeft),
        keys.pressed(KeyCode::ControlRight),
        keys.pressed(KeyCode::ShiftLeft),
        keys.pressed(KeyCode::ShiftRight),
    ];
    #[cfg(target_arch = "wasm32")]
    {
        let events = match crate::web_inputs::events() {
            Ok(events) => events,
            Err(problem) => {
                error.0 = problem;
                next.set(VnState::Error);
                return;
            }
        };
        if source.as_deref().is_some_and(|source|source.any()) || matches!(
            state.get(),
            VnState::Waiting | VnState::Stepping | VnState::Animating
        ) {
            for mut browser_event in events {
                let modal = screens
                    .views
                    .iter()
                    .rposition(|view| view.modal)
                    .unwrap_or(0);
                // A preceding handler may already have closed or covered the
                // field. Discard only those now-stale browser events.
                if !screens.views.iter().skip(modal).any(|view| {
                    view.name == browser_event.screen && view.root.available(&browser_event.element)
                }) {
                    continue;
                }
                if browser_event
                    .key
                    .as_deref()
                    .is_some_and(|key| matches!(key, "__rvn_next_focus" | "__rvn_previous_focus"))
                {
                    let order: Vec<_> = screens
                        .views
                        .iter()
                        .skip(modal)
                        .flat_map(|view| {
                            view.root
                                .focus_order()
                                .into_iter()
                                .map(|id| (view.name.clone(), id))
                        })
                        .collect();
                    if order.is_empty() {
                        continue;
                    }
                    let current = order
                        .iter()
                        .position(|(screen, element)| {
                            *screen == browser_event.screen && *element == browser_event.element
                        })
                        .unwrap_or(0);
                    let index = if browser_event.key.as_deref() == Some("__rvn_previous_focus") {
                        (current + order.len() - 1) % order.len()
                    } else {
                        (current + 1) % order.len()
                    };
                    browser_event = event(&order[index].0, &order[index].1, EventKind::Focus, None);
                }
                if !send(
                    &mut engine,
                    browser_event,
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                ) {
                    return;
                }
                screens.keyboard_consumed = true;
            }
            if crate::web_inputs::focused() {
                keyboard.clear();
                ime.clear();
                wheel.clear();
                screens.keyboard_consumed = true;
                if let Ok(mut window) = windows.get_single_mut() {
                    window.ime_enabled = false;
                }
                return;
            }
        }
    }
    if !source.as_deref().is_some_and(|source|source.any()) && !matches!(
        state.get(),
        VnState::Waiting | VnState::Stepping | VnState::Animating
    ) {
        keyboard.clear();
        ime.clear();
        wheel.clear();
        screens.dragging = None;
        if let Ok(mut window) = windows.get_single_mut() {
            window.ime_enabled = false;
        }
        return;
    }
    if let Some(drag) = screens.dragging.clone() {
        screens.pointer_consumed = true;
        if !mouse.pressed(MouseButton::Left) {
            screens.dragging = None;
        } else if let Some(view) = screens.views.iter().find(|view| view.name == drag.screen) {
            if let Some(component) = view
                .root
                .find(&drag.element)
                .cloned()
                .filter(|component| view.root.available(&component.id))
            {
                if let Some(cursor) = windows
                    .get_single()
                    .ok()
                    .and_then(|window| window.cursor_position())
                {
                    let Some((_, _, node, transform)) =
                        controls.iter().find(|(control, _, _, _)| {
                            control.option.is_none()
                                && control.screen == drag.screen
                                && control.element == drag.element
                        })
                    else {
                        screens.dragging = None;
                        return;
                    };
                    let Some(fraction) = slider_fraction(node.size(), transform, cursor) else {
                        screens.dragging = None;
                        return;
                    };
                    let value =
                        component.min + (component.max - component.min) * f64::from(fraction);
                    if component.value.as_f64() != Some(f64::from(value as f32)) {
                        if !send(
                            &mut engine,
                            event(
                                &drag.screen,
                                &drag.element,
                                EventKind::Change,
                                Some(Value::Float(value as f32)),
                            ),
                            &mut screens,
                            &mut output,
                            &mut error,
                            &mut next,
                        ) {
                            return;
                        }
                    }
                }
            } else {
                screens.dragging = None;
            }
        } else {
            screens.dragging = None;
        }
    }
    for (control, interaction, node, transform) in &controls {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(view) = screens
            .views
            .iter()
            .find(|view| view.name == control.screen)
        {
            if let Some(component) = view
                .root
                .find(&control.element)
                .filter(|component| component.kind == Kind::Canvas)
            {
                // Canvas input/capture has its own validated event path. Honor
                // explicit passthrough while disabled canvases still block hits.
                screens.pointer_consumed |=
                    component.consume_input || !view.root.available(&component.id);
                continue;
            }
        }
        screens.pointer_consumed = true;
        // Rebuilding a control while the pointer is held must not re-trigger
        // its click or toggle. A press is an edge, not a per-frame action.
        if !mouse.just_pressed(MouseButton::Left) {
            continue;
        }
        let Some(view) = screens
            .views
            .iter()
            .find(|view| view.name == control.screen)
            .cloned()
        else {
            continue;
        };
        let Some(component) = view.root.find(&control.element).cloned() else {
            continue;
        };
        if !view.root.available(&component.id)
            || screens
                .views
                .iter()
                .any(|other| other.modal && (other.layer, other.order) > (view.layer, view.order))
        {
            continue;
        }
        if let Some(option) = &control.option {
            screens.dropdown = None;
            send(
                &mut engine,
                event(
                    &control.screen,
                    &control.element,
                    EventKind::Change,
                    Some(Value::Str(option.clone())),
                ),
                &mut screens,
                &mut output,
                &mut error,
                &mut next,
            );
            continue;
        }
        if component.is_focusable() {
            if !send(
                &mut engine,
                event(&control.screen, &control.element, EventKind::Focus, None),
                &mut screens,
                &mut output,
                &mut error,
                &mut next,
            ) {
                return;
            }
        }
        if !screens
            .views
            .iter()
            .any(|view| view.name == control.screen && view.root.find(&control.element).is_some())
        {
            continue;
        }
        match component.kind {
            Kind::Input => {
                screens.editing = Editing {
                    screen: control.screen.clone(),
                    element: control.element.clone(),
                    caret: component.value.as_str().unwrap_or_default().len(),
                    ..default()
                };
            }
            Kind::Select => {
                screens.dropdown = Some(Dropdown {
                    screen: control.screen.clone(),
                    element: control.element.clone(),
                    options: component.options.clone(),
                    labels: component.option_labels.clone(),
                    highlighted: component
                        .options
                        .iter()
                        .position(|option| component.value.as_str() == Some(option))
                        .unwrap_or(0),
                    position: transform.translation().truncate()
                        + Vec2::new(-node.size().x * 0.5, node.size().y * 0.5),
                    width: node.size().x,
                });
            }
            Kind::Toggle => {
                send(
                    &mut engine,
                    event(
                        &control.screen,
                        &control.element,
                        EventKind::Change,
                        Some(Value::Bool(!component.value.as_bool().unwrap_or(false))),
                    ),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            }
            Kind::Slider => {
                screens.dragging = Some(Dragging {
                    screen: control.screen.clone(),
                    element: control.element.clone(),
                });
                if let Some(cursor) = windows
                    .get_single()
                    .ok()
                    .and_then(|window| window.cursor_position())
                {
                    let Some(fraction) = slider_fraction(node.size(), transform, cursor) else {
                        continue;
                    };
                    let value = component.min + (component.max - component.min) * fraction as f64;
                    send(
                        &mut engine,
                        event(
                            &control.screen,
                            &control.element,
                            EventKind::Change,
                            Some(Value::Float(value as f32)),
                        ),
                        &mut screens,
                        &mut output,
                        &mut error,
                        &mut next,
                    );
                }
            }
            Kind::Button => {
                send(
                    &mut engine,
                    event(&control.screen, &control.element, EventKind::Click, None),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            }
            _ => {}
        }
    }
    if screens.dropdown.is_some()
        && mouse.just_pressed(MouseButton::Left)
        && !screens.pointer_consumed
    {
        screens.dropdown = None;
        screens.pointer_consumed = true;
    }
    let modal = screens
        .views
        .iter()
        .rposition(|view| view.modal)
        .unwrap_or(0);
    let focus_order: Vec<_> = screens
        .views
        .iter()
        .skip(modal)
        .flat_map(|view| {
            view.root
                .focus_order()
                .into_iter()
                .map(|id| (view.name.clone(), id))
        })
        .collect();
    if !screens
        .keyboard_focus
        .as_ref()
        .is_some_and(|focus| focus_order.contains(focus))
    {
        screens.keyboard_focus = screens.views.iter().skip(modal).rev().find_map(|view| {
            view.focus
                .as_ref()
                .filter(|id| view.root.focus_order().contains(id))
                .map(|id| (view.name.clone(), id.clone()))
        });
    }
    let view = screens
        .keyboard_focus
        .as_ref()
        .and_then(|(name, _)| screens.views.iter().find(|view| &view.name == name))
        .or_else(|| screens.views.last())
        .cloned();
    let Some(view) = view else {
        keyboard.clear();
        ime.clear();
        wheel.clear();
        if let Ok(mut window) = windows.get_single_mut() {
            window.ime_enabled = false;
        }
        return;
    };
    let focused = screens
        .keyboard_focus
        .as_ref()
        .and_then(|(_, id)| view.root.find(id))
        .cloned();
    // Preserve modifiers at the time of each key event. A complete short
    // chord can arrive between two frames, after ButtonInput has released
    // Shift/Ctrl already; using only its final state reverses Shift+Tab.
    let shift = pressed
        .iter()
        .find(|(key, _, _)| key.key_code == KeyCode::Tab)
        .map(|(_, _, shift)| *shift)
        .unwrap_or(keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
    for (key, _, _) in &pressed {
        let target = focused
            .as_ref()
            .filter(|component| {
                component.kind != Kind::Canvas && component.events.contains_key(&EventKind::Key)
            })
            .or_else(|| {
                (view.root.kind != Kind::Canvas && view.root.events.contains_key(&EventKind::Key))
                    .then_some(&view.root)
            });
        if let Some(target) = target {
            let mut input = event(&view.name, &target.id, EventKind::Key, None);
            input.key = Some(match &key.logical_key {
                Key::Character(text) => text.to_string(),
                other => format!("{other:?}"),
            });
            if !send(
                &mut engine,
                input,
                &mut screens,
                &mut output,
                &mut error,
                &mut next,
            ) {
                return;
            }
            screens.keyboard_consumed = true;
        }
    }
    if !screens.views.iter().any(|screen| screen.name == view.name) {
        ime.clear();
        return;
    }
    if keys.just_pressed(KeyCode::Tab) && !focus_order.is_empty() {
        let current = screens
            .keyboard_focus
            .as_ref()
            .and_then(|focus| focus_order.iter().position(|other| other == focus));
        let index = current.map_or(if shift { focus_order.len() - 1 } else { 0 }, |current| {
            if shift {
                (current + focus_order.len() - 1) % focus_order.len()
            } else {
                (current + 1) % focus_order.len()
            }
        });
        let (name, id) = &focus_order[index];
        if !send(
            &mut engine,
            event(name, id, EventKind::Focus, None),
            &mut screens,
            &mut output,
            &mut error,
            &mut next,
        ) {
            return;
        }
        screens.keyboard_focus = Some((name.clone(), id.clone()));
        screens.reveal_focus = Some((name.clone(), id.clone()));
        screens.dropdown = None;
        screens.keyboard_consumed = true;
        keyboard.clear();
        ime.clear();
        wheel.clear();
        return;
    }
    if let Some(mut menu) = screens.dropdown.clone() {
        screens.keyboard_consumed = true;
        if !menu.options.is_empty() {
            for scroll in wheel.read() {
                let delta = match scroll.unit {
                    MouseScrollUnit::Line => scroll.y,
                    MouseScrollUnit::Pixel => scroll.y / 36.0,
                };
                if delta != 0.0 {
                    menu.highlighted = (menu.highlighted as isize - delta.signum() as isize)
                        .clamp(0, menu.options.len() as isize - 1)
                        as usize;
                }
            }
            if keys.just_pressed(KeyCode::ArrowDown) {
                menu.highlighted = (menu.highlighted + 1) % menu.options.len();
            }
            if keys.just_pressed(KeyCode::ArrowUp) {
                menu.highlighted = (menu.highlighted + menu.options.len() - 1) % menu.options.len();
            }
            if keys.just_pressed(KeyCode::Enter) {
                screens.dropdown = None;
                send(
                    &mut engine,
                    event(
                        &menu.screen,
                        &menu.element,
                        EventKind::Change,
                        Some(Value::Str(menu.options[menu.highlighted].clone())),
                    ),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            } else if keys.just_pressed(KeyCode::Escape) {
                screens.dropdown = None;
            } else {
                screens.dropdown = Some(menu);
            }
        }
        keyboard.clear();
        ime.clear();
        return;
    }
    wheel.clear();
    if let Some(component) = focused.clone() {
        if component.kind == Kind::Input {
            screens.keyboard_consumed = true;
            let mut text = component.value.as_str().unwrap_or_default().to_owned();
            if screens.editing.screen != view.name || screens.editing.element != component.id {
                screens.editing = Editing {
                    screen: view.name.clone(),
                    element: component.id.clone(),
                    caret: text.len(),
                    ..default()
                };
            }
            let mut changed = false;
            let ime_events: Vec<_> = ime.read().cloned().collect();
            if ime_events
                .iter()
                .any(|event| matches!(event, Ime::Commit { .. } | Ime::Preedit { .. }))
            {
                screens.editing.composing = true;
            }
            for (input, control, key_shift) in &pressed {
                changed |= edit_text(&mut text, &mut screens.editing, input, *control, *key_shift);
            }
            for event in &ime_events {
                match event {
                    Ime::Preedit { value, .. } => screens.editing.composing = !value.is_empty(),
                    Ime::Commit { value, .. } => {
                        replace_selection(&mut text, &mut screens.editing, value);
                        changed = true;
                        screens.editing.composing = false;
                    }
                    _ => {}
                }
            }
            if let Ok(mut window) = windows.get_single_mut() {
                window.ime_enabled = true;
            }
            if changed {
                send(
                    &mut engine,
                    event(
                        &view.name,
                        &component.id,
                        EventKind::Change,
                        Some(Value::Str(text)),
                    ),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            }
        } else {
            if let Ok(mut window) = windows.get_single_mut() {
                window.ime_enabled = false;
            }
            if component.kind == Kind::Slider
                && (keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowRight))
            {
                screens.keyboard_consumed = true;
                let direction = if keys.just_pressed(KeyCode::ArrowLeft) {
                    -1.0
                } else {
                    1.0
                };
                let value = (component.value.as_f64().unwrap_or(component.min)
                    + direction * (component.max - component.min) / 100.0)
                    .clamp(component.min, component.max);
                send(
                    &mut engine,
                    event(
                        &view.name,
                        &component.id,
                        EventKind::Change,
                        Some(Value::Float(value as f32)),
                    ),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            }
            if component.kind == Kind::Select
                && (keys.just_pressed(KeyCode::Enter)
                    || keys.just_pressed(KeyCode::Space)
                    || keys.just_pressed(KeyCode::ArrowDown))
            {
                screens.keyboard_consumed = true;
                if let Some((_, _, node, transform)) = controls.iter().find(|(control, _, _, _)| {
                    control.screen == view.name
                        && control.element == component.id
                        && control.option.is_none()
                }) {
                    screens.dropdown = Some(Dropdown {
                        screen: view.name.clone(),
                        element: component.id.clone(),
                        options: component.options.clone(),
                        labels: component.option_labels.clone(),
                        highlighted: component
                            .options
                            .iter()
                            .position(|option| component.value.as_str() == Some(option))
                            .unwrap_or(0),
                        position: transform.translation().truncate()
                            + Vec2::new(-node.size().x * 0.5, node.size().y * 0.5),
                        width: node.size().x,
                    });
                }
            } else if (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space))
                && (component.kind != Kind::Canvas
                    || component.events.contains_key(&EventKind::Activate))
            {
                screens.keyboard_consumed = true;
                let kind = if component.kind == Kind::Toggle {
                    EventKind::Change
                } else {
                    EventKind::Activate
                };
                let value = if component.kind == Kind::Toggle {
                    Some(Value::Bool(!component.value.as_bool().unwrap_or(false)))
                } else {
                    None
                };
                send(
                    &mut engine,
                    event(&view.name, &component.id, kind, value),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            }
            keyboard.clear();
            ime.clear();
        }
    } else {
        keyboard.clear();
        ime.clear();
        if let Ok(mut window) = windows.get_single_mut() {
            window.ime_enabled = false;
        }
    }
    if keys.just_pressed(KeyCode::Escape)
        || keys.just_pressed(KeyCode::F5)
        || keys.just_pressed(KeyCode::F6)
    {
        if !view.root.events.contains_key(&EventKind::Key)
            && !focused.as_ref().is_some_and(|component| {
                component.kind != Kind::Canvas && component.events.contains_key(&EventKind::Key)
            })
        {
            screens.keyboard_consumed = false;
        }
    }
}

pub(crate) fn key_chords(
    events: impl Iterator<Item = KeyboardInput>,
    modifiers: &mut [bool; 4],
) -> Vec<(KeyboardInput, bool, bool)> {
    let mut pressed = Vec::new();
    for event in events {
        let index = match event.key_code {
            KeyCode::ControlLeft => Some(0),
            KeyCode::ControlRight => Some(1),
            KeyCode::ShiftLeft => Some(2),
            KeyCode::ShiftRight => Some(3),
            _ => None,
        };
        if let Some(index) = index {
            modifiers[index] = event.state == ButtonState::Pressed;
        }
        if event.state == ButtonState::Pressed {
            pressed.push((
                event,
                modifiers[0] || modifiers[1],
                modifiers[2] || modifiers[3],
            ));
        }
    }
    pressed
}

#[cfg(test)]
mod chord_tests {
    use super::*;
    fn key(code: KeyCode, state: ButtonState) -> KeyboardInput {
        KeyboardInput {
            key_code: code,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            window: Entity::PLACEHOLDER,
        }
    }
    #[test]
    fn short_chords_use_modifiers_at_the_key_press_not_after_release() {
        let mut held = [false; 4];
        let events = vec![
            key(KeyCode::ShiftLeft, ButtonState::Pressed),
            key(KeyCode::Tab, ButtonState::Pressed),
            key(KeyCode::Tab, ButtonState::Released),
            key(KeyCode::ShiftLeft, ButtonState::Released),
        ];
        let pressed = key_chords(events.into_iter(), &mut held);
        assert!(
            pressed
                .iter()
                .find(|(key, _, _)| key.key_code == KeyCode::Tab)
                .unwrap()
                .2
        );
        assert_eq!(held, [false; 4]);
        let events = vec![
            key(KeyCode::ControlLeft, ButtonState::Pressed),
            key(KeyCode::KeyA, ButtonState::Pressed),
            key(KeyCode::ControlLeft, ButtonState::Released),
        ];
        assert!(
            key_chords(events.into_iter(), &mut held)
                .iter()
                .find(|(key, _, _)| key.key_code == KeyCode::KeyA)
                .unwrap()
                .1
        );
    }
    #[test]
    fn a_modifier_held_across_frames_is_preserved() {
        let mut held = [false; 4];
        key_chords(
            vec![key(KeyCode::ShiftRight, ButtonState::Pressed)].into_iter(),
            &mut held,
        );
        assert!(
            key_chords(
                vec![key(KeyCode::Tab, ButtonState::Pressed)].into_iter(),
                &mut held
            )[0]
            .2
        );
        key_chords(
            vec![key(KeyCode::ShiftRight, ButtonState::Released)].into_iter(),
            &mut held,
        );
        assert_eq!(held, [false; 4]);
    }
    #[test]
    fn a_later_modifier_press_does_not_rewrite_an_earlier_key() {
        let mut held = [false; 4];
        let events = vec![
            key(KeyCode::Tab, ButtonState::Pressed),
            key(KeyCode::ShiftLeft, ButtonState::Pressed),
        ];
        assert!(!key_chords(events.into_iter(), &mut held)[0].2);
    }
}

fn replace_selection(text: &mut String, editing: &mut Editing, inserted: &str) {
    editing.caret = editing.caret.min(text.len());
    while !text.is_char_boundary(editing.caret) {
        editing.caret -= 1;
    }
    let anchor = editing
        .anchor
        .take()
        .unwrap_or(editing.caret)
        .min(text.len());
    let mut start = anchor.min(editing.caret);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let end = anchor.max(editing.caret);
    if !text.is_char_boundary(end) {
        return;
    }
    text.replace_range(start..end, inserted);
    editing.caret = start + inserted.len();
}

fn edit_text(
    text: &mut String,
    editing: &mut Editing,
    input: &KeyboardInput,
    control: bool,
    shift: bool,
) -> bool {
    let mut caret = editing.caret.min(text.len());
    while !text.is_char_boundary(caret) {
        caret -= 1;
    }
    editing.caret = caret;
    let previous = text[..caret]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0);
    let following = text[caret..]
        .chars()
        .next()
        .map(|c| caret + c.len_utf8())
        .unwrap_or(text.len());
    let move_to = match input.key_code {
        KeyCode::ArrowLeft => Some(previous),
        KeyCode::ArrowRight => Some(following),
        KeyCode::Home => Some(0),
        KeyCode::End => Some(text.len()),
        _ => None,
    };
    if let Some(next) = move_to {
        if shift && editing.anchor.is_none() {
            editing.anchor = Some(caret);
        }
        if !shift {
            editing.anchor = None;
        }
        editing.caret = next;
        return false;
    }
    if control && input.key_code == KeyCode::KeyA {
        editing.anchor = Some(0);
        editing.caret = text.len();
        return false;
    }
    if input.key_code == KeyCode::Backspace || input.key_code == KeyCode::Delete {
        if editing.anchor.is_none() {
            editing.anchor = Some(if input.key_code == KeyCode::Backspace {
                previous
            } else {
                following
            });
        }
        replace_selection(text, editing, "");
        return true;
    }
    if !control && !editing.composing {
        if let Key::Character(value) = &input.logical_key {
            if !value.chars().any(char::is_control) {
                replace_selection(text, editing, value);
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::mouse::{mouse_button_input_system, MouseButtonInput};

    // A windowless schedule retains real Bevy mouse/keyboard event reduction,
    // both focus passes and the actual RVN input dispatcher. Geometry is a
    // completed-layout fixture; no Interaction or handler result is injected.
    fn pointer_app() -> (App, Entity, Vec<Entity>) {
        let script=rvn_parser::parse(r#"
handler clicked(event) { set clicks = clicks + 1 }
screen form() {
    return {"id":"root","kind":"panel","children":[
        {"id":"button","kind":"button","enabled":enabled,"events":{"click":"clicked","activate":"clicked"}},
        {"id":"toggle","kind":"toggle","binding":"toggled"},
        {"id":"slider","kind":"slider","binding":"level","min":0,"max":1},
        {"id":"name","kind":"input","binding":"name"}
    ]}
}
init { set clicks=0 set enabled=true set toggled=false set level=0 set name="Alix" }
label start
    ui.open("form",[],false,0)
    "Waiting"
"#).unwrap();
        let mut engine =
            rvn_core::Engine::new(script, crate::bevy_renderer::BevyRenderer::new(), 32).unwrap();
        engine.step_until_interaction().unwrap();
        let mut screens = Screens {
            active: true,
            ..default()
        };
        update_views(&mut screens, engine.interface_views().unwrap());
        let mut app = App::new();
        app.insert_resource(VnEngine(engine))
            .insert_resource(screens)
            .insert_resource(State::new(VnState::Waiting))
            .init_resource::<NextState<VnState>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<bevy::input::touch::Touches>()
            .init_resource::<bevy::ui::UiScale>()
            .init_resource::<bevy::ui::UiStack>()
            .init_resource::<ScriptErrorMessage>()
            .init_resource::<crate::accessibility::Accessibility>()
            .add_event::<MouseButtonInput>()
            .add_event::<KeyboardInput>()
            .add_event::<bevy::input::keyboard::KeyboardFocusLost>()
            .add_event::<Ime>()
            .add_event::<MouseWheel>()
            .add_event::<VnCommand>()
            .add_systems(
                PreUpdate,
                (
                    mouse_button_input_system,
                    bevy::input::keyboard::keyboard_input_system,
                    bevy::ui::ui_focus_system,
                    transformed_focus,
                )
                    .chain(),
            )
            .add_systems(Update, input);
        let window = app
            .world_mut()
            .spawn((Window::default(), bevy::window::PrimaryWindow))
            .id();
        let camera = app
            .world_mut()
            .spawn((Camera::default(), GlobalTransform::default()))
            .id();
        let mut controls = Vec::new();
        for (index, id) in ["button", "toggle", "slider", "name"]
            .into_iter()
            .enumerate()
        {
            controls.push(pointer_node(
                app.world_mut(),
                Some(id),
                camera,
                Vec2::new(100.0, 80.0 + index as f32 * 80.0),
                Vec2::new(120.0, 40.0),
            ));
        }
        app.world_mut().resource_mut::<bevy::ui::UiStack>().uinodes = controls.clone();
        (app, window, controls)
    }

    fn story_menu_pointer_app()->(App,Entity) {
        let source=r#"
handler open_inventory(event){ui.open_story("form",[],false,60)}
handler bad(event){set clicks=clicks+1 local roll=random(1,10) ui.focus("form","name") menu.execute({"kind":"bool_preference","key":"fullscreen","value":1})}
handler bad_constructor(event){set clicks=clicks+1 menu.execute(menu_bool("fullscreen",1))}
handler good(event){set clicks=clicks+1 menu.execute(menu_action("none"))}
handler general_fault(event){local missing=unknown_scenario_variable}
screen host(){return component("open","button",{"events":{"click":"open_inventory"}},[])}
screen form(){return component("root","panel",{},[
    component("button","button",{"events":{"click":"bad"}},[]),
    component("toggle","button",{"events":{"click":"good"}},[]),
    component("slider","button",{"events":{"click":"bad_constructor"}},[]),
    component("name","input",{"binding":"name"},[]),
    component("general","button",{"events":{"click":"general_fault"}},[])
])}
init{set clicks=0 set name="Alix"}
label start
"Waiting"
"#;
        let mut engine=rvn_core::Engine::new(rvn_parser::parse(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        engine.step_until_interaction().unwrap();
        engine.synchronize_source_menu(rvn_ui::PageRole::QuickActions,Some("host"),Value::Dict(Default::default()),false,100).unwrap();
        engine.interface_event(event("host","open",EventKind::Click,None)).unwrap();
        engine.synchronize_source_menu(rvn_ui::PageRole::QuickActions,None,Value::Dict(Default::default()),false,0).unwrap();
        assert_eq!(engine.state.ui.screens.len(),1);assert_eq!(engine.state.ui.screens[0].host_role,None);
        engine.renderer.menu_authority.story_ui_active=true;engine.renderer.menu_authority.game_active=true;engine.renderer.menu_authority.waiting=true;
        engine.renderer.take_pending();
        let views=engine.interface_views().unwrap();let(mut app,window,controls)=pointer_app();
        let camera=app.world().get::<bevy::ui::TargetCamera>(controls[0]).unwrap().entity();
        let extra=pointer_node(app.world_mut(),Some("general"),camera,Vec2::new(100.0,400.0),Vec2::new(120.0,40.0));
        app.world_mut().resource_mut::<bevy::ui::UiStack>().uinodes.push(extra);
        app.insert_resource(VnEngine(engine));update_views(&mut app.world_mut().resource_mut::<Screens>(),views);
        (app,window)
    }

    #[test]
    fn narrative_menu_rejections_preserve_globals_random_controls_disk_and_next_real_click() {
        let(mut app,window)=story_menu_pointer_app();
        let directory=std::env::temp_dir().join(format!("rvn-narrative-menu-rejection-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let manager=rvn_core::save::SaveManager::new(&directory,10).unwrap();
        app.world().resource::<VnEngine>().0.save(&manager,1,"Preserved".into(),"qa.rvn".into()).unwrap();
        let disk=||std::fs::read_dir(&directory).unwrap().map(|entry|{let entry=entry.unwrap();(entry.file_name(),std::fs::read(entry.path()).unwrap())}).collect::<std::collections::BTreeMap<_,_>>();
        let before_disk=disk();let before=app.world().resource::<VnEngine>().0.state.clone();
        let controls=|state:&rvn_core::GameState|serde_json::json!(state.ui.screens.iter().map(|screen|(&screen.name,&screen.values,&screen.canvas_states)).collect::<Vec<_>>());
        for y in [80.0,240.0] {
            pointer_move(&mut app,window,Vec2::new(100.0,y));pointer_edge(&mut app,window,ButtonState::Pressed);pointer_edge(&mut app,window,ButtonState::Released);app.update();
            let engine=&app.world().resource::<VnEngine>().0;
            assert_eq!(engine.state.vars,before.vars);assert_eq!(engine.state.random,before.random);assert_eq!(engine.state.display_random,before.display_random);
            assert_eq!(controls(&engine.state),controls(&before));assert_eq!(disk(),before_disk);
            assert!(matches!(app.world().resource::<NextState<VnState>>(),NextState::Unchanged));
            assert!(!app.world().resource::<ScriptErrorMessage>().0.is_empty());assert!(!engine.renderer.menu_pending);
        }
        pointer_move(&mut app,window,Vec2::new(100.0,160.0));pointer_edge(&mut app,window,ButtonState::Pressed);pointer_edge(&mut app,window,ButtonState::Released);app.update();
        assert_eq!(test_var(&app,"clicks"),Value::Int(1));assert!(app.world().resource::<VnEngine>().0.renderer.menu_pending);
        assert!(app.world().resource::<Events<VnCommand>>().iter_current_update_events().any(|event|matches!(event,VnCommand::SourceMenu(receipt)if receipt.effect.host_role.is_none()&&receipt.effect.screen=="form")));
        assert_eq!(disk(),before_disk);std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn general_fault_in_an_ordinary_interface_keeps_the_fatal_story_error_path() {
        let(mut app,window)=story_menu_pointer_app();
        pointer_move(&mut app,window,Vec2::new(100.0,400.0));pointer_edge(&mut app,window,ButtonState::Pressed);pointer_edge(&mut app,window,ButtonState::Released);app.update();
        assert!(matches!(app.world().resource::<NextState<VnState>>(),NextState::Pending(VnState::Error)));
        assert!(app.world().resource::<ScriptErrorMessage>().0.contains("unknown_scenario_variable"));
    }

    fn pointer_node(
        world: &mut World,
        id: Option<&str>,
        camera: Entity,
        center: Vec2,
        size: Vec2,
    ) -> Entity {
        let mut node = Node::default();
        let bevy::reflect::ReflectMut::Struct(fields) = node.reflect_mut() else {
            unreachable!()
        };
        fields.field_mut("calculated_size").unwrap().apply(&size);
        let mut visible = ViewVisibility::default();
        visible.set();
        let entity = world
            .spawn((
                node,
                GlobalTransform::from_translation(center.extend(0.0)),
                visible,
                FocusPolicy::Block,
                Interaction::None,
                bevy::ui::TargetCamera(camera),
            ))
            .id();
        if let Some(id) = id {
            world.entity_mut(entity).insert(Control {
                screen: "form".into(),
                element: id.into(),
                option: None,
            });
        }
        entity
    }

    fn pointer_move(app: &mut App, window: Entity, point: Vec2) {
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(point));
    }
    fn pointer_edge(app: &mut App, window: Entity, state: ButtonState) {
        app.world_mut().send_event(MouseButtonInput {
            window,
            button: MouseButton::Left,
            state,
        });
    }
    fn short_click(app: &mut App, window: Entity, point: Vec2) {
        pointer_move(app, window, point);
        pointer_edge(app, window, ButtonState::Pressed);
        pointer_edge(app, window, ButtonState::Released);
        app.update();
        assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    fn test_var(app: &App, name: &str) -> Value {
        app.world().resource::<VnEngine>().0.state.vars[name].clone()
    }
    fn key_edge(app: &mut App, window: Entity, code: KeyCode, logical: Key) {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            app.world_mut().send_event(KeyboardInput {
                window,
                key_code: code,
                logical_key: logical.clone(),
                state,
            });
        }
        app.update();
    }

    #[test]
    fn authored_screen_short_shift_tab_preserves_both_directions_and_modifier_event_order() {
        use ButtonState::{Pressed, Released};
        fn edges(app: &mut App, window: Entity, sequence: &[(KeyCode, ButtonState)]) {
            for &(code, state) in sequence {
                app.world_mut().send_event(KeyboardInput {
                    key_code: code,
                    logical_key: if code == KeyCode::Tab { Key::Tab } else { Key::Shift },
                    state,
                    window,
                });
            }
            app.update();
            assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
        }
        fn focused(app: &App) -> Option<(&str, &str)> {
            app.world().resource::<Screens>().keyboard_focus.as_ref()
                .map(|(screen, element)| (screen.as_str(), element.as_str()))
        }
        for shift in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
            let (mut app, window, _) = pointer_app();
            edges(&mut app, window, &[(shift, Pressed), (KeyCode::Tab, Pressed),
                (KeyCode::Tab, Released), (shift, Released)]);
            assert_eq!(focused(&app), Some(("form", "name")));
            assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(shift));
            edges(&mut app, window, &[(KeyCode::Tab, Pressed), (KeyCode::Tab, Released)]);
            assert_eq!(focused(&app), Some(("form", "button")));
            edges(&mut app, window, &[(shift, Pressed), (KeyCode::Tab, Pressed),
                (KeyCode::Tab, Released), (shift, Released)]);
            assert_eq!(focused(&app), Some(("form", "name")));
            edges(&mut app, window, &[(shift, Pressed), (shift, Released),
                (KeyCode::Tab, Pressed), (KeyCode::Tab, Released)]);
            assert_eq!(focused(&app), Some(("form", "button")));
            edges(&mut app, window, &[(KeyCode::Tab, Pressed), (shift, Pressed),
                (KeyCode::Tab, Released), (shift, Released)]);
            assert_eq!(focused(&app), Some(("form", "toggle")));
            edges(&mut app, window, &[(shift, Pressed)]);
            edges(&mut app, window, &[(KeyCode::Tab, Pressed), (KeyCode::Tab, Released)]);
            assert_eq!(focused(&app), Some(("form", "button")));
            edges(&mut app, window, &[(shift, Released), (KeyCode::Tab, Pressed),
                (KeyCode::Tab, Released)]);
            assert_eq!(focused(&app), Some(("form", "toggle")));
        }
    }

    #[test]
    fn same_frame_mouse_down_up_activates_once_through_real_focus_and_input_schedule() {
        let (mut app, window, controls) = pointer_app();
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        let mouse = app.world().resource::<ButtonInput<MouseButton>>();
        assert!(!mouse.pressed(MouseButton::Left));
        assert!(mouse.just_pressed(MouseButton::Left) && mouse.just_released(MouseButton::Left));
        assert_eq!(
            test_var(&app, "clicks"),
            Value::Int(1),
            "A complete short click between frames must reach its actual RVN handler"
        );
        app.update();
        assert_eq!(test_var(&app, "clicks"), Value::Int(1));
        assert_eq!(
            *app.world().get::<Interaction>(controls[0]).unwrap(),
            Interaction::Hovered
        );
        pointer_edge(&mut app, window, ButtonState::Pressed);
        app.update();
        app.update();
        assert_eq!(
            test_var(&app, "clicks"),
            Value::Int(2),
            "Holding the pointer across frames must not repeat a click"
        );
        pointer_move(&mut app, window, Vec2::new(100.0, 160.0));
        pointer_edge(&mut app, window, ButtonState::Released);
        app.update();
        assert_eq!(
            test_var(&app, "toggled"),
            Value::Bool(false),
            "Releasing a prior press over another control must not activate that control"
        );
        short_click(&mut app, window, Vec2::new(100.0, 160.0));
        assert_eq!(test_var(&app, "toggled"), Value::Bool(true));
        app.update();
        assert_eq!(test_var(&app, "toggled"), Value::Bool(true));
        short_click(&mut app, window, Vec2::new(130.0, 240.0));
        assert_eq!(test_var(&app, "level"), Value::Float(0.75));
        pointer_move(&mut app, window, Vec2::new(45.0, 240.0));
        app.update();
        assert_eq!(
            test_var(&app, "level"),
            Value::Float(0.75),
            "A released short slider click must not retain a drag"
        );
    }

    #[test]
    fn short_click_respects_disabled_blocked_hidden_and_modal_controls() {
        let (mut app, window, controls) = pointer_app();
        app.world_mut()
            .resource_mut::<VnEngine>()
            .0
            .state
            .vars
            .insert("enabled".into(), Value::Bool(false));
        let views = app
            .world()
            .resource::<VnEngine>()
            .0
            .interface_views()
            .unwrap();
        update_views(&mut app.world_mut().resource_mut::<Screens>(), views);
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        assert_eq!(test_var(&app, "clicks"), Value::Int(0));
        app.world_mut()
            .resource_mut::<crate::accessibility::Accessibility>()
            .blocked = true;
        short_click(&mut app, window, Vec2::new(100.0, 160.0));
        assert_eq!(test_var(&app, "toggled"), Value::Bool(false));
        app.world_mut()
            .resource_mut::<crate::accessibility::Accessibility>()
            .blocked = false;
        *app.world_mut()
            .get_mut::<ViewVisibility>(controls[1])
            .unwrap() = ViewVisibility::default();
        short_click(&mut app, window, Vec2::new(100.0, 160.0));
        assert_eq!(test_var(&app, "toggled"), Value::Bool(false));
        app.world_mut()
            .get_mut::<ViewVisibility>(controls[1])
            .unwrap()
            .set();
        let camera = app
            .world()
            .get::<bevy::ui::TargetCamera>(controls[1])
            .unwrap()
            .entity();
        let overlay = pointer_node(
            app.world_mut(),
            None,
            camera,
            Vec2::new(100.0, 160.0),
            Vec2::new(120.0, 40.0),
        );
        app.world_mut()
            .resource_mut::<bevy::ui::UiStack>()
            .uinodes
            .push(overlay);
        short_click(&mut app, window, Vec2::new(100.0, 160.0));
        assert_eq!(
            test_var(&app, "toggled"),
            Value::Bool(false),
            "A blocking overlay must absorb the short click"
        );
        app.world_mut()
            .resource_mut::<bevy::ui::UiStack>()
            .uinodes
            .pop();
        let root = UiComponent::parse(serde_json::json!({"id":"cover","kind":"panel"})).unwrap();
        app.world_mut()
            .resource_mut::<Screens>()
            .views
            .push(ScreenView {
                name: "modal".into(),
                modal: true,
                layer: 10,
                order: 10,
                focus: None,
                root,
            });
        short_click(&mut app, window, Vec2::new(100.0, 160.0));
        assert_eq!(
            test_var(&app, "toggled"),
            Value::Bool(false),
            "A higher authored modal must block underlying controls"
        );
    }

    #[test]
    fn short_mouse_click_preserves_keyboard_activation_tab_unicode_and_ime() {
        let (mut app, window, _) = pointer_app();
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        key_edge(&mut app, window, KeyCode::Enter, Key::Enter);
        assert_eq!(test_var(&app, "clicks"), Value::Int(2));
        key_edge(&mut app, window, KeyCode::Tab, Key::Tab);
        assert_eq!(
            app.world().resource::<Screens>().keyboard_focus,
            Some(("form".into(), "toggle".into()))
        );
        short_click(&mut app, window, Vec2::new(100.0, 320.0));
        key_edge(&mut app, window, KeyCode::KeyQ, Key::Character("q".into()));
        assert_eq!(test_var(&app, "name"), Value::Str("Alixq".into()));
        app.world_mut().send_event(Ime::Commit {
            window,
            value: "é".into(),
        });
        app.update();
        assert_eq!(test_var(&app, "name"), Value::Str("Alixqé".into()));
    }

    #[test]
    fn legacy_title_choice_and_pause_menu_keep_bevys_complete_short_click() {
        let (mut app, window, controls) = pointer_app();
        app.world_mut()
            .entity_mut(controls[0])
            .remove::<Control>()
            .insert((Button, crate::components::ChoiceButton(3)));
        app.init_resource::<crate::menu_documents::Menus>()
            .init_resource::<crate::resources::ChoiceFocus>()
            .add_event::<crate::vn_command::PlayerInput>()
            .add_systems(
                Update,
                crate::systems::choice_interaction_system.after(input),
            );
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        let mut reader = app
            .world()
            .resource::<Events<crate::vn_command::PlayerInput>>()
            .get_reader();
        let events = app
            .world()
            .resource::<Events<crate::vn_command::PlayerInput>>();
        assert_eq!(
            reader
                .read(events)
                .filter(|event| matches!(event, crate::vn_command::PlayerInput::Choose(3)))
                .count(),
            1
        );
        app.update();
        assert_eq!(
            reader
                .read(
                    app.world()
                        .resource::<Events<crate::vn_command::PlayerInput>>()
                )
                .count(),
            0
        );

        let (mut app, window, controls) = pointer_app();
        app.world_mut()
            .entity_mut(controls[0])
            .remove::<Control>()
            .insert((
                Button,
                crate::systems::menu::MenuButton::Save,
                BackgroundColor(Color::NONE),
            ));
        app.init_resource::<crate::resources::MenuState>()
            .init_resource::<crate::systems::save_menu::SaveMenuState>()
            .init_resource::<crate::systems::settings_menu::SettingsMenuState>()
            .add_event::<bevy::app::AppExit>()
            .add_systems(Update, crate::systems::menu_interaction_system.after(input));
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        assert!(
            app.world()
                .resource::<crate::systems::save_menu::SaveMenuState>()
                .active
        );
        app.update();
        assert_eq!(
            *app.world().get::<Interaction>(controls[0]).unwrap(),
            Interaction::Hovered
        );

        let (mut app, window, controls) = pointer_app();
        app.world_mut()
            .entity_mut(controls[0])
            .remove::<Control>()
            .insert((
                Button,
                crate::systems::title::TitleButton::Settings,
                crate::systems::title::TitleButtonColors {
                    normal: Color::NONE,
                    hover: Color::NONE,
                    pressed: Color::NONE,
                },
                BackgroundColor(Color::NONE),
            ));
        let empty = std::path::PathBuf::new();
        app.insert_resource(crate::project_paths::ProjectPaths::new(
            empty.clone(),
            empty.clone(),
            empty.clone(),
            empty.clone(),
            empty,
        ))
        .insert_resource(crate::resources::PersistentDataResource {
            // Settings activation neither reads nor writes this manager.
            manager: rvn_core::PersistentDataManager::new(std::env::temp_dir()).unwrap(),
            data: rvn_core::PersistentData::default(),
        })
        .init_resource::<crate::resources::VnRenderState>()
        .init_resource::<crate::resources::ImagemapState>()
        .init_resource::<crate::resources::TypewriterState>()
        .init_resource::<crate::resources::DialogueHistory>()
        .init_resource::<crate::resources::MenuState>()
        .init_resource::<crate::systems::save_menu::SaveMenuState>()
        .init_resource::<crate::systems::settings_menu::SettingsMenuState>()
        .add_event::<bevy::app::AppExit>()
        .add_systems(
            Update,
            crate::systems::title_interaction_system.after(input),
        );
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        assert!(
            app.world()
                .resource::<crate::systems::settings_menu::SettingsMenuState>()
                .active
        );
        assert_eq!(
            app.world()
                .resource::<crate::resources::MenuState>()
                .return_to,
            Some(VnState::TitleScreen)
        );
    }

    #[test]
    fn canvas_frames_refresh_without_rebuilding_host_widgets() {
        use super::*;
        use rvn_ui::custom_canvas::{CanvasDrawing, CanvasFrame, CanvasPrimitive};
        let mut component = UiComponent::parse(
            serde_json::json!({"kind":"canvas","id":"canvas","width":300,"height":180}),
        )
        .unwrap();
        component.canvas_frame = Some(CanvasFrame {
            width: 300.0,
            height: 180.0,
            time: 0.0,
        });
        let first = vec![ScreenView {
            name: "example".into(),
            modal: false,
            layer: 0,
            order: 0,
            focus: None,
            root: component,
        }];
        let mut second = first.clone();
        second[0].root.canvas_frame.as_mut().unwrap().time = 2.0;
        second[0].root.drawing = Some(CanvasDrawing {
            primitives: vec![CanvasPrimitive::Rect {
                rect: [0.0, 0.0, 10.0, 10.0],
                color: [1.0; 4],
                radius: 0.0,
            }],
        });
        assert!(same_host_views(&first, &second));
        second[0].root.canvas_frame.as_mut().unwrap().width = 400.0;
        assert!(!same_host_views(&first, &second));
        second = first.clone();
        second[0].root.enabled = false;
        assert!(!same_host_views(&first, &second));
        second = first.clone();
        second[0].root.font_size = 42.0;
        assert!(!same_host_views(&first, &second));
    }
    #[test]
    fn authored_small_fonts_follow_reference_scale_without_an_arbitrary_readability_floor() {
        use super::*;
        let component = UiComponent::parse(
            serde_json::json!({"id":"tiny","kind":"input","font_size":6,"value":"Small"}),
        )
        .unwrap();
        assert_eq!(
            component_font_size(component.font_size, 720.0 / 1080.0),
            4.0
        );
        assert_eq!(component_font_size(component.font_size, 1.0), 6.0);
        assert_eq!(component_font_size(component.font_size, 0.1), 1.0);
        assert_eq!(component_font_size(24.0, 720.0 / 1080.0), 16.0);
    }
    #[test]
    fn ui_shader_fix_is_exact_idempotent_and_rejects_unknown_dependencies() {
        use super::*;
        let old="// Geometry and flags unchanged\nfn antialias(distance:f32)->f32{return clamp(0.0, 1.0, 0.5 - 2.0 * distance);}";
        let corrected = corrected_ui_antialias(old).unwrap().unwrap();
        assert_eq!(
            corrected,
            old.replace(
                "clamp(0.0, 1.0, 0.5 - 2.0 * distance)",
                "clamp(0.5 - 2.0 * distance, 0.0, 1.0)"
            )
        );
        assert_eq!(corrected_ui_antialias(&corrected).unwrap(), None);
        assert!(corrected_ui_antialias("unrecognized shader").is_err());
        assert!(corrected_ui_antialias(&format!("{old}\n{old}")).is_err());
        assert!(corrected_ui_antialias(&format!("{corrected}\n{corrected}")).is_err());
        for (distance, alpha) in [(-1.0f32, 1.0f32), (0.0, 0.5), (1.0, 0.0)] {
            assert_eq!((0.5 - 2.0 * distance).clamp(0.0, 1.0), alpha);
        }
    }
    #[test]
    fn designer_and_runtime_share_nested_geometry_and_keep_scroll_content_reachable() {
        use super::*;
        let root=UiComponent::parse(serde_json::json!({"id":"root","kind":"column","rect":[100,100,300,240],"padding":10,"children":[{"id":"nested","kind":"panel","width":80,"height":60,"children":[{"id":"caption","kind":"text","rect":[4,6,20,10]}]}]})).unwrap();
        let view = ScreenView {
            name: "form".into(),
            modal: false,
            layer: 0,
            order: 0,
            focus: None,
            root,
        };
        let resolved = layout_view(&view);
        let rects = layout_rects(&view.root, [1920.0, 1080.0]).unwrap();
        let nested = rects.iter().find(|item| item.id == "nested").unwrap().rect;
        assert_eq!(resolved.root.rect, Some([100.0, 100.0, 300.0, 240.0]));
        assert_eq!(
            resolved.root.find("caption").unwrap().rect,
            Some([4.0, 6.0, 20.0, 10.0])
        );
        assert_eq!(
            resolved.root.find("nested").unwrap().rect,
            Some([nested[0] - 100.0, nested[1] - 100.0, nested[2], nested[3]])
        );
        let root=UiComponent::parse(serde_json::json!({"id":"root","kind":"column","width":300,"children":[{"id":"long","kind":"text","height":3000}]})).unwrap();
        let view = ScreenView { root, ..view };
        let resolved = layout_view(&view);
        assert_eq!(resolved.root.scroll, ScrollAxis::Both);
        assert_eq!(resolved.root.rect.unwrap()[3], 1048.0);
        assert_eq!(resolved.root.find("long").unwrap().rect.unwrap()[3], 3000.0);
    }

    #[test]
    fn accessibility_text_size_reflows_auto_controls_without_scaling_authored_fonts_twice() {
        use super::*;
        let root=UiComponent::parse(serde_json::json!({"id":"root","kind":"column","children":[{"id":"caption","kind":"text","text":"Title"},{"id":"action","kind":"button","text":"Continue"}]})).unwrap();
        let view = ScreenView {
            name: "form".into(),
            modal: false,
            layer: 0,
            order: 0,
            focus: None,
            root,
        };
        let normal = layout_view_with_text_scale(&view, 1.0);
        let enlarged = layout_view_with_text_scale(&view, 2.0);
        assert!(
            enlarged.root.find("action").unwrap().rect.unwrap()[3]
                > normal.root.find("action").unwrap().rect.unwrap()[3]
        );
        assert_eq!(enlarged.root.find("action").unwrap().font_size, 24.0);
    }

    #[test]
    fn custom_feedback_colors_include_inherited_opacity_and_visible_focus() {
        use super::*;
        let root=UiComponent::parse(serde_json::json!({"id":"root","kind":"column","opacity":0.5,"children":[{"id":"action","kind":"button","opacity":0.5,"hover_background":[0.8,0.2,0.1,0.6],"focus_color":[0.1,0.9,0.2,1]}]})).unwrap();
        let mut app = App::new();
        app.init_resource::<Screens>().add_systems(Update, feedback);
        app.world_mut()
            .resource_mut::<Screens>()
            .views
            .push(ScreenView {
                name: "form".into(),
                modal: false,
                layer: 0,
                order: 0,
                focus: Some("action".into()),
                root,
            });
        let entity = app
            .world_mut()
            .spawn((
                Control {
                    screen: "form".into(),
                    element: "action".into(),
                    option: None,
                },
                Interaction::Hovered,
                BackgroundColor(Color::NONE),
                BorderColor(Color::NONE),
            ))
            .id();
        app.update();
        let background = app
            .world()
            .get::<BackgroundColor>(entity)
            .unwrap()
            .0
            .to_srgba();
        let border = app.world().get::<BorderColor>(entity).unwrap().0.to_srgba();
        assert!((background.red - 0.8).abs() < 0.001);
        assert!((background.alpha - 0.15).abs() < 0.001);
        assert!((border.green - 0.9).abs() < 0.001);
        assert!((border.alpha - 0.25).abs() < 0.001);
    }

    #[test]
    fn assistive_actions_validate_values_and_match_pointer_events() {
        use super::*;
        use bevy::a11y::accesskit::{Action, ActionData, ActionRequest, NodeId};
        let request = |action, data| ActionRequest {
            target: NodeId(1),
            action,
            data,
        };
        let component = |kind: &str, extra: serde_json::Value| {
            let mut value = serde_json::json!({"id":"control","kind":kind});
            value
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            serde_json::from_value::<UiComponent>(value).unwrap()
        };
        let button = component("button", serde_json::json!({}));
        assert_eq!(
            assistive_event("inventory", &button, &request(Action::Default, None))
                .unwrap()
                .kind,
            EventKind::Activate
        );
        let toggle = component("toggle", serde_json::json!({"value":true}));
        assert_eq!(
            assistive_event("inventory", &toggle, &request(Action::Default, None))
                .unwrap()
                .value,
            Some(Value::Bool(false))
        );
        let select = component(
            "select",
            serde_json::json!({"options":["letter","key"],"value":"letter"}),
        );
        assert!(assistive_event(
            "inventory",
            &select,
            &request(Action::SetValue, Some(ActionData::Value("missing".into())))
        )
        .is_none());
        assert_eq!(
            assistive_event(
                "inventory",
                &select,
                &request(Action::SetValue, Some(ActionData::Value("key".into())))
            )
            .unwrap()
            .value,
            Some(Value::Str("key".into()))
        );
        let slider = component("slider", serde_json::json!({"min":0,"max":1,"value":0.5}));
        assert_eq!(
            assistive_event(
                "inventory",
                &slider,
                &request(Action::SetValue, Some(ActionData::NumericValue(50.0)))
            )
            .unwrap()
            .value,
            Some(Value::Float(1.0))
        );
        assert!(assistive_event(
            "inventory",
            &slider,
            &request(Action::SetValue, Some(ActionData::NumericValue(f64::NAN)))
        )
        .is_none());
        let input = component("input", serde_json::json!({"value":""}));
        for invalid in ["\0".to_owned(), "x".repeat(65_537)] {
            assert!(assistive_event(
                "inventory",
                &input,
                &request(
                    Action::SetValue,
                    Some(ActionData::Value(invalid.into_boxed_str()))
                )
            )
            .is_none());
        }
        assert_eq!(
            assistive_event(
                "inventory",
                &input,
                &request(Action::SetValue, Some(ActionData::Value("Éloïse".into())))
            )
            .unwrap()
            .value,
            Some(Value::Str("Éloïse".into()))
        );
        let canvas = component("canvas", serde_json::json!({}));
        assert!(assistive_event("inventory", &canvas, &request(Action::Default, None)).is_none());
        assert!(assistive_event(
            "inventory",
            &canvas,
            &request(Action::SetValue, Some(ActionData::Value("ignored".into())))
        )
        .is_none());
        let canvas = component(
            "canvas",
            serde_json::json!({"events":{"activate":"custom_activate"}}),
        );
        assert_eq!(
            assistive_event("inventory", &canvas, &request(Action::Default, None))
                .unwrap()
                .kind,
            EventKind::Activate
        );
    }
    #[test]
    fn animated_hit_testing_and_slider_drag_use_the_rendered_affine_transform() {
        use super::*;
        let transform = GlobalTransform::from(Transform {
            translation: Vec3::new(300.0, 200.0, 0.0),
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            scale: Vec3::new(2.0, 0.5, 1.0),
        });
        let size = Vec2::new(100.0, 40.0);
        assert!(transformed_contains(
            size,
            &transform,
            Vec2::new(300.0, 270.0)
        ));
        assert!(!transformed_contains(
            size,
            &transform,
            Vec2::new(320.0, 200.0)
        ));
        assert!(
            (slider_fraction(size, &transform, Vec2::new(300.0, 250.0)).unwrap() - 0.75).abs()
                < 0.001
        );
        assert!(local_pointer(
            &GlobalTransform::from(Transform::from_scale(Vec3::ZERO)),
            Vec2::ZERO
        )
        .is_none());
    }
    #[test]
    fn text_selection_preserves_unicode_boundaries() {
        let mut text = "Éloïse".to_owned();
        let mut editing = Editing {
            caret: 2,
            anchor: Some(0),
            ..default()
        };
        replace_selection(&mut text, &mut editing, "Å");
        assert_eq!(text, "Åloïse");
        assert_eq!(editing.caret, 2);
    }
}
