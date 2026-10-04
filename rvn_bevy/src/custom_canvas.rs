//! Creator-defined RVN drawings use a single, portable Bevy UI material.
//! Saved state, simulation and validation belong to core; this module owns only
//! GPU assets, font masks, transient pointer capture and platform input routing.
use ab_glyph::{Font as _, ScaleFont as _};
use bevy::{
    prelude::*,
    reflect::TypePath,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{
            AsBindGroup, Extent3d, ShaderRef, ShaderType, TextureDimension, TextureFormat,
        },
    },
    ui::{FocusPolicy, OverflowAxis, UiMaterial, UiMaterialPlugin},
};
use std::collections::{HashMap, HashSet};
// Bevy's derive macros do not resolve a target-specific umbrella dependency.
// These are its public reexports, not extra copies of any Bevy crate.
use crate::resources::{ScriptErrorMessage, VnState};
use crate::{programmable_ui::Screens, resources::VnEngine, vn_command::VnCommand};
use bevy::render::render_resource::encase;
use bevy::{asset as bevy_asset, reflect as bevy_reflect, render as bevy_render};
use bevy::{
    ecs::event::ManualEventReader,
    input::{
        keyboard::{Key, KeyboardInput},
        mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel},
        touch::{TouchInput, TouchPhase},
        ButtonState,
    },
    window::{CursorMoved, WindowFocused},
};
use rvn_core::ui::UiInput;
use rvn_parser::Value;
use rvn_ui::custom_canvas::{
    flatten, inverse_point, transform_point, CanvasClip, CanvasDrawing, CanvasItem,
    CanvasPrimitive, MAX_DRAW_POINTS,
};
use rvn_ui::programmable::{ComponentKind, ScreenEventKind};

const SHADER: Handle<Shader> = Handle::weak_from_u128(0x09fe783edf7340f9a7a113c74405ad90);
const MAX_CLIPS: usize = 64;
const MAX_TEXT_AXIS: u32 = 4096;
const MAX_TEXT_PIXELS: usize = 16_777_216;
const MAX_GLYPH_PIXELS: usize = 64 * 1024 * 1024;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CanvasRenderSet;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CanvasInputSet;

pub(crate) struct CustomCanvasPlugin;
impl Plugin for CustomCanvasPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::load_internal_asset!(app, SHADER, "custom_canvas.wgsl", Shader::from_wgsl);
        app.add_plugins(UiMaterialPlugin::<CanvasMaterial>::default());
        let white = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new_fill(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[255, 255, 255, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::all(),
            ));
        app.insert_resource(CanvasCache { white, ..default() })
            .init_resource::<PointerState>()
            .add_systems(
                Update,
                (
                    tick.after(crate::programmable_ui::InterfaceReceiveSet)
                        .before(crate::programmable_ui::InterfaceInputSet),
                    input
                        .after(crate::programmable_ui::InterfaceInputSet)
                        .before(crate::programmable_ui::InterfaceRenderSet)
                        .before(crate::systems::input_system)
                        .before(crate::systems::player_input_system)
                        .in_set(CanvasInputSet),
                    refresh_surfaces
                        .after(crate::programmable_ui::InterfaceRenderSet)
                        .before(build)
                        .in_set(CanvasRenderSet),
                    build.after(refresh_surfaces).in_set(CanvasRenderSet),
                    refresh_texture_bindings
                        .after(build)
                        .in_set(CanvasRenderSet),
                ),
            )
            .add_systems(
                PostUpdate,
                sync_materials
                    .after(bevy::ui::UiSystem::Layout)
                    .after(bevy::ui::UiSystem::Stack)
                    .after(bevy::transform::TransformSystem::TransformPropagate)
                    .after(bevy::ui::update::update_clipping_system)
                    .after(crate::programmable_ui::InterfaceScrollSet),
            );
    }
}

/// Geometry is in authored local reference pixels, not logical window pixels.
/// The host UI resolves the extent once; animated transforms are inherited.
#[derive(Component, Clone)]
pub(crate) struct CanvasSurface {
    pub screen: String,
    pub element: String,
    pub drawing: CanvasDrawing,
    pub extent: Vec2,
    pub font: Handle<Font>,
    pub opacity: f32,
}
#[derive(Component)]
struct Built;
#[derive(Component)]
struct CanvasLeaf {
    surface: Entity,
    item: CanvasItem,
    original_color: [f32; 4],
}
#[derive(Component)]
struct InkTint {
    original: [f32; 4],
    current: [f32; 4],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
struct ClipUniform {
    row_x: Vec4,
    row_y: Vec4,
    rect: Vec4,
}
#[derive(Clone, Debug, PartialEq, ShaderType)]
struct InkUniform {
    bounds: Vec4,
    row_x: Vec4,
    row_y: Vec4,
    color: Vec4,
    rect: Vec4,
    parameters: Vec4,
    counts: Vec4,
    points: [Vec4; MAX_DRAW_POINTS],
    clips: [ClipUniform; MAX_CLIPS],
}
impl Default for InkUniform {
    fn default() -> Self {
        Self {
            bounds: Vec4::ZERO,
            row_x: Vec4::ZERO,
            row_y: Vec4::ZERO,
            color: Vec4::ONE,
            rect: Vec4::ZERO,
            parameters: Vec4::ZERO,
            counts: Vec4::ZERO,
            points: [Vec4::ZERO; MAX_DRAW_POINTS],
            clips: [ClipUniform::default(); MAX_CLIPS],
        }
    }
}
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct CanvasMaterial {
    #[uniform(0)]
    ink: InkUniform,
    #[texture(1)]
    #[sampler(2)]
    texture: Handle<Image>,
}
impl UiMaterial for CanvasMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct TextKey {
    font: bevy::asset::AssetId<Font>,
    text: String,
    size: u32,
    quality: u32,
}
#[derive(Clone)]
struct TextMask {
    image: Handle<Image>,
    rect: [f32; 4],
    pixels: usize,
}
#[derive(Resource, Default)]
struct CanvasCache {
    white: Handle<Image>,
    // Keep async image requests alive even before fonts/geometry are ready.
    // Reissuing an unretained failed request every frame hides its failure.
    images: HashMap<String, Handle<Image>>,
    materials: HashMap<(String, String, usize), Handle<CanvasMaterial>>,
    text: HashMap<TextKey, TextMask>,
}

fn inverse_matrix(matrix: [f32; 6]) -> Option<[f32; 6]> {
    let origin = inverse_point(matrix, [0.0, 0.0])?;
    let x = inverse_point(matrix, [1.0, 0.0])?;
    let y = inverse_point(matrix, [0.0, 1.0])?;
    Some([
        x[0] - origin[0],
        x[1] - origin[1],
        y[0] - origin[0],
        y[1] - origin[1],
        origin[0],
        origin[1],
    ])
}
fn rows(matrix: [f32; 6]) -> (Vec4, Vec4) {
    (
        Vec4::new(matrix[0], matrix[2], matrix[4], 0.0),
        Vec4::new(matrix[1], matrix[3], matrix[5], 0.0),
    )
}
fn clip_uniform(clip: &CanvasClip) -> Option<ClipUniform> {
    let (row_x, row_y) = rows(inverse_matrix(clip.matrix)?);
    Some(ClipUniform {
        row_x,
        row_y,
        rect: Vec4::from_array(clip.rect),
    })
}
fn affine(matrix: Mat4) -> [f32; 6] {
    [
        matrix.x_axis.x,
        matrix.x_axis.y,
        matrix.y_axis.x,
        matrix.y_axis.y,
        matrix.w_axis.x,
        matrix.w_axis.y,
    ]
}
fn transformed_bounds(matrix: [f32; 6], rect: [f32; 4]) -> [f32; 4] {
    let corners = [
        [rect[0], rect[1]],
        [rect[0] + rect[2], rect[1]],
        [rect[0], rect[1] + rect[3]],
        [rect[0] + rect[2], rect[1] + rect[3]],
    ]
    .map(|point| Vec2::from_array(transform_point(matrix, point)));
    let low = corners
        .into_iter()
        .fold(Vec2::splat(f32::INFINITY), Vec2::min);
    let high = corners
        .into_iter()
        .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
    [low.x, low.y, high.x - low.x, high.y - low.y]
}
fn points_bounds(points: &[[f32; 2]], extra: f32) -> [f32; 4] {
    let low = points
        .iter()
        .map(|point| Vec2::from_array(*point))
        .fold(Vec2::splat(f32::INFINITY), Vec2::min)
        - Vec2::splat(extra);
    let high = points
        .iter()
        .map(|point| Vec2::from_array(*point))
        .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max)
        + Vec2::splat(extra);
    [low.x, low.y, high.x - low.x, high.y - low.y]
}
fn intersect_bounds(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let low = Vec2::new(a[0], a[1]).max(Vec2::new(b[0], b[1]));
    let high = Vec2::new(a[0] + a[2], a[1] + a[3]).min(Vec2::new(b[0] + b[2], b[1] + b[3]));
    [
        low.x,
        low.y,
        (high.x - low.x).max(0.0),
        (high.y - low.y).max(0.0),
    ]
}

/// FontArc, kerning, advances and outlines are the very assets/rasterizer used
/// by Bevy text. Masks are cached by font/string/size/HiDPI quality, never per
/// frame. Coordinates are top-left and explicit newlines use font line height.
fn text_mask(
    font: &Font,
    text: &str,
    size: f32,
    quality: f32,
) -> Result<(Image, [f32; 4]), String> {
    let scaled = font.font.as_scaled(size * quality);
    let line_height = scaled.height() + scaled.line_gap();
    let mut pen = Vec2::new(0.0, scaled.ascent());
    let mut previous = None;
    let mut glyphs = Vec::new();
    let mut low = Vec2::splat(f32::INFINITY);
    let mut high = Vec2::splat(f32::NEG_INFINITY);
    let mut glyph_pixels = 0usize;
    for character in text.chars() {
        if character == '\n' {
            pen.x = 0.0;
            pen.y += line_height;
            previous = None;
            continue;
        }
        if character == '\r' {
            continue;
        }
        let id = scaled.glyph_id(character);
        if let Some(previous) = previous {
            pen.x += scaled.kern(previous, id);
        }
        let mut glyph = id.with_scale(size * quality);
        glyph.position = ab_glyph::point(pen.x, pen.y);
        if let Some(outline) = font.font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            let width = (bounds.max.x - bounds.min.x).ceil().max(0.0) as usize;
            let height = (bounds.max.y - bounds.min.y).ceil().max(0.0) as usize;
            glyph_pixels = glyph_pixels
                .checked_add(
                    width
                        .checked_mul(height)
                        .ok_or_else(|| "Canvas glyph rasterization budget overflow".to_owned())?,
                )
                .ok_or_else(|| "Canvas glyph rasterization budget overflow".to_owned())?;
            if glyph_pixels > MAX_GLYPH_PIXELS {
                return Err("Canvas text exceeds the portable 64-megapixel glyph-rasterization work budget; split overlapping text into smaller commands".into());
            }
            low = low.min(Vec2::new(bounds.min.x, bounds.min.y));
            high = high.max(Vec2::new(bounds.max.x, bounds.max.y));
            glyphs.push(outline);
        }
        pen.x += scaled.h_advance(id);
        previous = Some(id);
        if !pen.is_finite() || pen.abs().max_element() > MAX_TEXT_AXIS as f32 {
            return Err("Canvas text exceeds the portable 4096-pixel font-mask extent; split it into smaller text commands".into());
        }
    }
    if glyphs.is_empty() {
        return Ok((
            Image::new_fill(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[255, 255, 255, 0],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::all(),
            ),
            [0.0, 0.0, 1.0 / quality, 1.0 / quality],
        ));
    }
    low = low.floor() - Vec2::ONE;
    high = high.ceil() + Vec2::ONE;
    let extent = (high - low).as_uvec2();
    if extent.max_element() > MAX_TEXT_AXIS
        || extent.x as usize * extent.y as usize > MAX_TEXT_PIXELS
    {
        return Err(
            "Canvas text exceeds the portable 4096-axis/16-megapixel font-mask budget".into(),
        );
    }
    let mut data = vec![0u8; extent.x as usize * extent.y as usize * 4];
    for glyph in glyphs {
        let bounds = glyph.px_bounds();
        let offset = Vec2::new(bounds.min.x, bounds.min.y) - low;
        glyph.draw(|x, y, coverage| {
            let x = x + offset.x as u32;
            let y = y + offset.y as u32;
            if x >= extent.x || y >= extent.y {
                return;
            }
            let offset = (y as usize * extent.x as usize + x as usize) * 4;
            let alpha = data[offset + 3] as f32 / 255.0;
            data[offset..offset + 3].fill(255);
            data[offset + 3] = ((coverage + alpha * (1.0 - coverage)) * 255.0).round() as u8;
        });
    }
    Ok((
        Image::new(
            Extent3d {
                width: extent.x,
                height: extent.y,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        ),
        [
            low.x / quality,
            low.y / quality,
            extent.x as f32 / quality,
            extent.y as f32 / quality,
        ],
    ))
}

fn text_key(surface: &CanvasSurface, text: &str, size: f32, quality: f32) -> TextKey {
    TextKey {
        font: surface.font.id(),
        text: text.into(),
        size: size.to_bits(),
        quality: quality.to_bits(),
    }
}
fn resource_problem(
    surface: &CanvasSurface,
    problem: &str,
    error: &mut ScriptErrorMessage,
    next: &mut NextState<VnState>,
) {
    error.0 = format!(
        "Screen '{}' / canvas '{}': {}",
        surface.screen, surface.element, problem
    );
    next.set(VnState::Error);
}

fn tick(
    time: Res<Time>,
    state: Res<State<VnState>>,
    windows: Query<&Window>,
    accessibility: Res<crate::accessibility::Accessibility>,
    mut engine: ResMut<VnEngine>,
    mut screens: ResMut<Screens>,
    mut output: EventWriter<VnCommand>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
    source: Option<Res<crate::source_menus::SourceMenus>>,
) {
    if accessibility.blocked
        || (!source.as_deref().is_some_and(|source|source.any()) && !matches!(
            state.get(),
            VnState::Waiting | VnState::Stepping | VnState::Animating
        ))
    {
        return;
    }
    if windows.get_single().is_ok_and(|window| !window.focused) {
        return;
    }
    let mut canvas = false;
    for view in &screens.views {
        view.root
            .visit(&mut |component| canvas |= component.kind == ComponentKind::Canvas);
    }
    if !canvas {
        return;
    }
    if let Err(problem) = engine
        .0
        .interface_tick(f64::from(time.delta_seconds()).min(0.25))
    {
        error.0 = rvn_core::error::ScriptError::from_runtime(&problem).to_string();
        next.set(VnState::Error);
        return;
    }
    for event in engine.0.renderer.take_pending() {
        if let VnCommand::Interfaces(views) = &event {
            crate::programmable_ui::update_views(&mut screens, views.clone());
        }
        output.send(event);
    }
}

#[derive(Clone, Debug)]
struct Capture {
    screen: String,
    element: String,
    order: u64,
    button: String,
    position: Vec2,
}
#[derive(Resource, Default)]
struct PointerState {
    mouse: ManualEventReader<MouseButtonInput>,
    motion: ManualEventReader<CursorMoved>,
    touch: ManualEventReader<TouchInput>,
    wheel: ManualEventReader<MouseWheel>,
    keyboard: ManualEventReader<KeyboardInput>,
    focus: ManualEventReader<WindowFocused>,
    captures: HashMap<String, Capture>,
    suppressed: HashSet<String>,
    epoch: u64,
    modifiers: [bool; 8],
}

fn modifiers_value(modifiers: [bool; 8]) -> Value {
    Value::Dict(std::collections::BTreeMap::from([
        ("ctrl".into(), Value::Bool(modifiers[0] || modifiers[1])),
        ("shift".into(), Value::Bool(modifiers[2] || modifiers[3])),
        ("alt".into(), Value::Bool(modifiers[4] || modifiers[5])),
        ("meta".into(), Value::Bool(modifiers[6] || modifiers[7])),
    ]))
}
fn pointer_payload(
    point: Vec2,
    pointer: &str,
    button: &str,
    captured: bool,
    modifiers: [bool; 8],
    hit: Option<String>,
    wheel: Vec2,
) -> Value {
    let mut value = std::collections::BTreeMap::from([
        ("x".into(), Value::Float(point.x)),
        ("y".into(), Value::Float(point.y)),
        ("pointer".into(), Value::Str(pointer.into())),
        ("button".into(), Value::Str(button.into())),
        ("captured".into(), Value::Bool(captured)),
        ("modifiers".into(), modifiers_value(modifiers)),
        ("dx".into(), Value::Float(wheel.x)),
        ("dy".into(), Value::Float(wheel.y)),
    ]);
    if let Some(hit) = hit {
        value.insert("hit".into(), Value::Str(hit));
    }
    Value::Dict(value)
}
fn button_name(button: MouseButton) -> String {
    match button {
        MouseButton::Left => "left".into(),
        MouseButton::Right => "right".into(),
        MouseButton::Middle => "middle".into(),
        MouseButton::Back => "back".into(),
        MouseButton::Forward => "forward".into(),
        MouseButton::Other(id) => format!("button:{id}"),
    }
}

/// The canvas and every scroll/clip ancestor are tested using inverse affine
/// transforms, rather than a conservative AABB. Captured moves deliberately
/// bypass viewport bounds while retaining these same local coordinates.
fn local_position(world: &World, entity: Entity, point: Vec2, captured: bool) -> Option<Vec2> {
    let surface = world.get::<CanvasSurface>(entity)?;
    let node = world.get::<Node>(entity)?;
    let transform = world.get::<GlobalTransform>(entity)?;
    if !node.size().cmpgt(Vec2::ZERO).all() {
        return None;
    }
    let matrix = transform.compute_matrix();
    if !matrix.is_finite() || matrix.determinant().abs() < 1e-8 {
        return None;
    }
    let local = matrix
        .inverse()
        .transform_point3(point.extend(transform.translation().z))
        .truncate();
    if !captured {
        if !local.abs().cmple(node.size() * 0.5).all() {
            return None;
        }
        let mut ancestor = entity;
        loop {
            let style = world.get::<Style>(ancestor)?;
            let transform = world.get::<GlobalTransform>(ancestor)?;
            let node = world.get::<Node>(ancestor)?;
            let matrix = transform.compute_matrix();
            if matrix.determinant().abs() < 1e-8 {
                return None;
            }
            let local = matrix
                .inverse()
                .transform_point3(point.extend(transform.translation().z))
                .truncate();
            if (style.overflow.x != OverflowAxis::Visible && local.x.abs() > node.size().x * 0.5)
                || (style.overflow.y != OverflowAxis::Visible
                    && local.y.abs() > node.size().y * 0.5)
            {
                return None;
            }
            let Some(parent) = world.get::<Parent>(ancestor) else {
                break;
            };
            ancestor = parent.get();
        }
    }
    let point = (local / node.size() + Vec2::splat(0.5)) * surface.extent;
    point.is_finite().then_some(point)
}
fn canvas_entity(world: &mut World, screen: &str, element: &str) -> Option<Entity> {
    world
        .query::<(Entity, &CanvasSurface)>()
        .iter(world)
        .find(|(_, surface)| surface.screen == screen && surface.element == element)
        .map(|(entity, _)| entity)
}
fn target_available(world: &World, capture: &Capture) -> bool {
    let screens = world.resource::<Screens>();
    screens
        .views
        .iter()
        .find(|view| view.name == capture.screen && view.order == capture.order)
        .is_some_and(|view| {
            view.root.available(&capture.element)
                && !screens.views.iter().any(|other| {
                    other.modal && (other.layer, other.order) > (view.layer, view.order)
                })
        })
}
fn pick(world: &World, point: Vec2) -> Option<(Entity, Capture, Vec2)> {
    let screens = world.resource::<Screens>();
    for entity in world.resource::<bevy::ui::UiStack>().uinodes.iter().rev() {
        if world
            .get::<ViewVisibility>(*entity)
            .is_some_and(|visible| !visible.get())
        {
            continue;
        }
        let Some(node) = world.get::<Node>(*entity) else {
            continue;
        };
        let Some(transform) = world.get::<GlobalTransform>(*entity) else {
            continue;
        };
        let matrix = transform.compute_matrix();
        if matrix.determinant().abs() < 1e-8 {
            continue;
        }
        let local = matrix
            .inverse()
            .transform_point3(point.extend(transform.translation().z))
            .truncate();
        if !local.abs().cmple(node.size() * 0.5).all() {
            continue;
        }
        if let Some(surface) = world.get::<CanvasSurface>(*entity) {
            let view = screens
                .views
                .iter()
                .find(|view| view.name == surface.screen)?;
            let capture = Capture {
                screen: surface.screen.clone(),
                element: surface.element.clone(),
                order: view.order,
                button: String::new(),
                position: point,
            };
            let local = local_position(world, *entity, point, false)?;
            if target_available(world, &capture) {
                return Some((*entity, capture, local));
            }
            return None;
        }
        if world
            .get::<crate::programmable_ui::Control>(*entity)
            .is_some()
            || world.get::<FocusPolicy>(*entity) == Some(&FocusPolicy::Block)
        {
            return None;
        }
    }
    None
}
fn dispatch(world: &mut World, event: UiInput) -> bool {
    if world.resource::<VnEngine>().0.renderer.menu_pending {return false;}
    let source_event=world.resource::<VnEngine>().0.state.ui.screens.iter().any(|screen|screen.name==event.screen && screen.host_role.is_some());
    if let Err(problem) = world.resource_mut::<VnEngine>().0.interface_event(event) {
        world.resource_mut::<ScriptErrorMessage>().0 =
            rvn_core::error::ScriptError::from_runtime(&problem).to_string();
        if source_event||problem.is_menu_request_rejection() {warn!("Game menu canvas event: {}",world.resource::<ScriptErrorMessage>().0);}
        else {world.resource_mut::<NextState<VnState>>().set(VnState::Error);}
        return false;
    }
    for event in world.resource_mut::<VnEngine>().0.renderer.take_pending() {
        if let VnCommand::Interfaces(views) = &event {
            crate::programmable_ui::update_views(
                &mut world.resource_mut::<Screens>(),
                views.clone(),
            );
        }
        world.send_event(event);
    }
    true
}
fn deliver(
    world: &mut World,
    capture: &Capture,
    pointer: &str,
    kind: ScreenEventKind,
    point: Vec2,
    captured: bool,
    modifiers: [bool; 8],
    wheel: Vec2,
) -> bool {
    let Some(entity) = canvas_entity(world, &capture.screen, &capture.element) else {
        return true;
    };
    let Some(local) = local_position(world, entity, point, captured) else {
        return true;
    };
    let Some(component) = world
        .resource::<Screens>()
        .views
        .iter()
        .find(|view| view.name == capture.screen)
        .and_then(|view| view.root.find(&capture.element))
        .cloned()
    else {
        return true;
    };
    let hit = component
        .drawing
        .as_ref()
        .and_then(|drawing| flatten(drawing).ok())
        .and_then(|items| rvn_ui::custom_canvas::hit_test(&items, local.to_array()));
    if component.consume_input {
        world.resource_mut::<Screens>().pointer_consumed = true;
    }
    dispatch(
        world,
        UiInput {
            screen: capture.screen.clone(),
            element: capture.element.clone(),
            kind,
            value: Some(pointer_payload(
                local,
                pointer,
                &capture.button,
                captured,
                modifiers,
                hit,
                wheel,
            )),
            key: None,
        },
    )
}
fn route_pointer(
    world: &mut World,
    state: &mut PointerState,
    pointer: &str,
    kind: ScreenEventKind,
    point: Vec2,
    button: &str,
    wheel: Vec2,
) -> bool {
    if state.suppressed.contains(pointer) && kind != ScreenEventKind::Wheel {
        if kind == ScreenEventKind::PointerDown {
            state.suppressed.remove(pointer);
        } else {
            if matches!(
                kind,
                ScreenEventKind::PointerUp | ScreenEventKind::PointerCancel
            ) {
                state.suppressed.remove(pointer);
            }
            return true;
        }
    }
    let existing = state.captures.get(pointer).cloned();
    let target = existing.clone().or_else(|| {
        pick(world, point).map(|(_, mut capture, _)| {
            capture.button = button.into();
            capture
        })
    });
    let Some(mut capture) = target else {
        return true;
    };
    capture.position = point;
    if !button.is_empty() && existing.is_none() {
        capture.button = button.into();
    }
    if kind == ScreenEventKind::PointerDown && existing.is_none() {
        let component = world
            .resource::<Screens>()
            .views
            .iter()
            .find(|view| view.name == capture.screen)
            .and_then(|view| view.root.find(&capture.element))
            .cloned();
        if let Some(component) = component {
            if component.is_focusable() {
                if !dispatch(
                    world,
                    UiInput {
                        screen: capture.screen.clone(),
                        element: capture.element.clone(),
                        kind: ScreenEventKind::Focus,
                        value: None,
                        key: None,
                    },
                ) {
                    return false;
                }
                world.resource_mut::<Screens>().keyboard_focus =
                    Some((capture.screen.clone(), capture.element.clone()));
            }
            if component.capture_pointer {
                state.captures.insert(pointer.into(), capture.clone());
            }
        }
    } else if existing.is_some() {
        state.captures.insert(pointer.into(), capture.clone());
    }
    let captured = state.captures.contains_key(pointer);
    let mut success = deliver(
        world,
        &capture,
        pointer,
        kind,
        point,
        captured,
        state.modifiers,
        wheel,
    );
    if success
        && captured
        && kind != ScreenEventKind::PointerCancel
        && !target_available(world, &capture)
    {
        success = deliver(
            world,
            &capture,
            pointer,
            ScreenEventKind::PointerCancel,
            point,
            true,
            state.modifiers,
            Vec2::ZERO,
        );
        state.captures.remove(pointer);
    }
    if kind == ScreenEventKind::PointerCancel
        || (kind == ScreenEventKind::PointerUp && (button == capture.button || button == "touch"))
    {
        state.captures.remove(pointer);
    }
    success
}

fn input(world: &mut World) {
    let mut pointer = world.remove_resource::<PointerState>().unwrap_or_default();
    let mouse: Vec<_> = pointer
        .mouse
        .read(world.resource::<Events<MouseButtonInput>>())
        .copied()
        .collect();
    let moves: Vec<_> = pointer
        .motion
        .read(world.resource::<Events<CursorMoved>>())
        .cloned()
        .collect();
    let touches: Vec<_> = pointer
        .touch
        .read(world.resource::<Events<TouchInput>>())
        .copied()
        .collect();
    let wheel: Vec<_> = pointer
        .wheel
        .read(world.resource::<Events<MouseWheel>>())
        .copied()
        .collect();
    let keys: Vec<_> = pointer
        .keyboard
        .read(world.resource::<Events<KeyboardInput>>())
        .cloned()
        .collect();
    let lost_focus = pointer
        .focus
        .read(world.resource::<Events<WindowFocused>>())
        .any(|event| !event.focused);
    let epoch = world.resource::<VnEngine>().0.interface_epoch();
    if epoch != pointer.epoch {
        pointer.suppressed.extend(pointer.captures.keys().cloned());
        pointer.captures.clear();
        pointer.epoch = epoch;
    }
    let blocked = world
        .resource::<crate::accessibility::Accessibility>()
        .blocked;
    let playing = world.get_resource::<crate::source_menus::SourceMenus>().is_some_and(|source|source.any()) || matches!(
        world.resource::<State<VnState>>().get(),
        VnState::Waiting | VnState::Stepping | VnState::Animating
    );
    let stale: Vec<_> = pointer
        .captures
        .iter()
        .filter(|(_, capture)| {
            lost_focus || blocked || !playing || !target_available(world, capture)
        })
        .map(|(id, capture)| (id.clone(), capture.clone()))
        .collect();
    for (id, capture) in stale {
        // A still-existing hidden/covered canvas may handle cancellation. A
        // closed or restored instance is never revived or modified by capture.
        let _ = deliver(
            world,
            &capture,
            &id,
            ScreenEventKind::PointerCancel,
            capture.position,
            true,
            pointer.modifiers,
            Vec2::ZERO,
        );
        pointer.captures.remove(&id);
    }
    if !playing || blocked || !world.resource::<Screens>().active {
        let held = world.resource::<ButtonInput<KeyCode>>();
        pointer.modifiers = [
            held.pressed(KeyCode::ControlLeft),
            held.pressed(KeyCode::ControlRight),
            held.pressed(KeyCode::ShiftLeft),
            held.pressed(KeyCode::ShiftRight),
            held.pressed(KeyCode::AltLeft),
            held.pressed(KeyCode::AltRight),
            held.pressed(KeyCode::SuperLeft),
            held.pressed(KeyCode::SuperRight),
        ];
        world.insert_resource(pointer);
        return;
    }
    #[cfg(target_arch = "wasm32")]
    if crate::web_inputs::focused() {
        pointer.captures.clear();
        world.insert_resource(pointer);
        return;
    }
    let cursor = world
        .query::<&Window>()
        .iter(world)
        .next()
        .and_then(Window::cursor_position);
    let mut success = true;
    // Mouse movement is a final sample per frame; touch paths retain all phase
    // events. The native/browser window system supplies pointer capture outside
    // the window canvas; RVN target capture remains a separate stable identity.
    if let Some(event) = moves.last() {
        success = route_pointer(
            world,
            &mut pointer,
            "mouse",
            ScreenEventKind::PointerMove,
            event.position,
            "",
            Vec2::ZERO,
        );
    }
    if success {
        for event in mouse {
            let position = cursor.or_else(|| {
                pointer
                    .captures
                    .get("mouse")
                    .map(|capture| capture.position)
            });
            let Some(position) = position else {
                continue;
            };
            let kind = if event.state == ButtonState::Pressed {
                ScreenEventKind::PointerDown
            } else {
                ScreenEventKind::PointerUp
            };
            if !route_pointer(
                world,
                &mut pointer,
                "mouse",
                kind,
                position,
                &button_name(event.button),
                Vec2::ZERO,
            ) {
                success = false;
                break;
            }
        }
    }
    if success {
        for event in touches {
            let kind = match event.phase {
                TouchPhase::Started => ScreenEventKind::PointerDown,
                TouchPhase::Moved => ScreenEventKind::PointerMove,
                TouchPhase::Ended => ScreenEventKind::PointerUp,
                TouchPhase::Canceled => ScreenEventKind::PointerCancel,
            };
            if !route_pointer(
                world,
                &mut pointer,
                &format!("touch:{}", event.id),
                kind,
                event.position,
                "touch",
                Vec2::ZERO,
            ) {
                success = false;
                break;
            }
        }
    }
    if success {
        if let Some(position) = cursor {
            for event in wheel {
                let delta = Vec2::new(event.x, event.y)
                    * if event.unit == MouseScrollUnit::Line {
                        36.0
                    } else {
                        1.0
                    };
                if !route_pointer(
                    world,
                    &mut pointer,
                    "mouse",
                    ScreenEventKind::Wheel,
                    position,
                    "",
                    delta,
                ) {
                    success = false;
                    break;
                }
            }
        }
    }
    for key in keys {
        let index = match key.key_code {
            KeyCode::ControlLeft => Some(0),
            KeyCode::ControlRight => Some(1),
            KeyCode::ShiftLeft => Some(2),
            KeyCode::ShiftRight => Some(3),
            KeyCode::AltLeft => Some(4),
            KeyCode::AltRight => Some(5),
            KeyCode::SuperLeft => Some(6),
            KeyCode::SuperRight => Some(7),
            _ => None,
        };
        if let Some(index) = index {
            pointer.modifiers[index] = key.state == ButtonState::Pressed;
        }
        if !success || reserved_game_key(key.key_code) {
            continue;
        }
        let focused = world.resource::<Screens>().keyboard_focus.clone();
        let Some((screen, element)) = focused else {
            continue;
        };
        let component = world
            .resource::<Screens>()
            .views
            .iter()
            .find(|view| view.name == screen)
            .and_then(|view| view.root.find(&element))
            .cloned();
        let Some(component) = component.filter(|component| {
            component.kind == ComponentKind::Canvas
                && component.events.contains_key(&ScreenEventKind::Key)
        }) else {
            continue;
        };
        let logical = match key.logical_key {
            Key::Character(text) => text.to_string(),
            other => format!("{other:?}"),
        };
        if component.consume_input {
            world.resource_mut::<Screens>().keyboard_consumed = true;
        }
        let value = Value::Dict(std::collections::BTreeMap::from([
            (
                "pressed".into(),
                Value::Bool(key.state == ButtonState::Pressed),
            ),
            ("code".into(), Value::Str(format!("{:?}", key.key_code))),
            ("modifiers".into(), modifiers_value(pointer.modifiers)),
        ]));
        success = dispatch(
            world,
            UiInput {
                screen,
                element,
                kind: ScreenEventKind::Key,
                value: Some(value),
                key: Some(logical),
            },
        );
    }
    if lost_focus {
        pointer.modifiers = [false; 8];
    }
    world.insert_resource(pointer);
}

/// A focused drawing must not make game navigation or quick saves inaccessible.
/// Screen-level handlers remain able to override these through the ordinary UI
/// dispatcher; custom canvas handlers receive all other press/release events.
fn reserved_game_key(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::Tab | KeyCode::Escape | KeyCode::F5 | KeyCode::F6
    )
}

fn refresh_surfaces(
    mut commands: Commands,
    screens: Res<Screens>,
    mut surfaces: Query<(Entity, &mut CanvasSurface)>,
    leaves: Query<(Entity, &CanvasLeaf)>,
) {
    for (entity, mut surface) in &mut surfaces {
        let drawing = screens
            .views
            .iter()
            .find(|view| view.name == surface.screen)
            .and_then(|view| view.root.find(&surface.element))
            .and_then(|component| component.drawing.as_ref());
        if let Some(drawing) = drawing.filter(|drawing| *drawing != &surface.drawing) {
            surface.drawing = drawing.clone();
            commands.entity(entity).remove::<Built>();
            for (leaf, description) in &leaves {
                if description.surface == entity {
                    commands.entity(leaf).despawn_recursive();
                }
            }
        }
    }
}
fn refresh_texture_bindings(
    mut events: EventReader<bevy::asset::AssetEvent<Image>>,
    mut materials: ResMut<Assets<CanvasMaterial>>,
) {
    let changed: HashSet<_> = events
        .read()
        .filter_map(|event| match event {
            bevy::asset::AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    if changed.is_empty() {
        return;
    }
    let handles: Vec<_> = materials
        .iter()
        .filter(|(_, material)| changed.contains(&material.texture.id()))
        .map(|(id, _)| id)
        .collect();
    // A replaced Image GPU view requires rebuilding its material bind group.
    // Merely retaining the same AssetId is insufficient for hot-reloaded art.
    for handle in handles {
        let _ = materials.get_mut(handle);
    }
}

fn build(
    mut commands: Commands,
    surfaces: Query<(Entity, &CanvasSurface, Option<&Built>, &Style)>,
    windows: Query<&Window>,
    assets: Res<AssetServer>,
    fonts: Res<Assets<Font>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<CanvasMaterial>>,
    mut cache: ResMut<CanvasCache>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let factor = Vec2::new(window.width() / 1920.0, window.height() / 1080.0);
    let quality = ((factor.max_element() * window.resolution.scale_factor()).clamp(1.0, 4.0) * 4.0)
        .ceil()
        / 4.0;
    let mut active_materials = HashSet::new();
    let mut active_text = HashSet::new();
    let mut active_images = HashSet::new();
    for (_, surface, _, _) in &surfaces {
        if let Ok(items) = flatten(&surface.drawing) {
            for (index, item) in items.iter().enumerate() {
                active_materials.insert((surface.screen.clone(), surface.element.clone(), index));
                if let CanvasPrimitive::Text { text, size, .. } = &item.primitive {
                    active_text.insert(text_key(surface, text, *size, quality));
                }
                if let CanvasPrimitive::Image { asset, .. } = &item.primitive {
                    active_images.insert(asset.clone());
                }
            }
        }
    }
    cache
        .materials
        .retain(|key, _| active_materials.contains(key));
    cache.text.retain(|key, _| active_text.contains(key));
    cache
        .images
        .retain(|asset, _| active_images.contains(asset));
    for asset in active_images {
        cache
            .images
            .entry(asset.clone())
            .or_insert_with(|| assets.load(asset));
    }
    for (entity, surface, built, host_style) in &surfaces {
        // Failed resources remain failures even after the geometry was built.
        if let Some(bevy::asset::LoadState::Failed(problem)) =
            assets.get_load_state(surface.font.id())
        {
            resource_problem(
                surface,
                &format!("Font could not be loaded: {problem}"),
                &mut error,
                &mut next,
            );
            return;
        }
        let items = match flatten(&surface.drawing) {
            Ok(items) => items,
            Err(problem) => {
                resource_problem(surface, &problem, &mut error, &mut next);
                return;
            }
        };
        for item in &items {
            if let CanvasPrimitive::Image { asset, .. } = &item.primitive {
                let handle = &cache.images[asset];
                if let Some(bevy::asset::LoadState::Failed(problem)) =
                    assets.get_load_state(handle.id())
                {
                    resource_problem(
                        surface,
                        &format!("Image '{asset}' could not be loaded: {problem}"),
                        &mut error,
                        &mut next,
                    );
                    return;
                }
            }
        }
        if built.is_some() {
            continue;
        }
        if items
            .iter()
            .any(|item| matches!(item.primitive, CanvasPrimitive::Text { .. }))
            && !fonts.contains(&surface.font)
        {
            continue;
        }
        let mut prepared = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            if matches!(item.primitive, CanvasPrimitive::Hit { .. }) {
                continue;
            }
            // A zero-scale group intentionally has no visible or clickable area.
            let Some(inverse) = inverse_matrix(item.matrix) else {
                continue;
            };
            if item.opacity <= 0.0 {
                continue;
            }
            let mut ink = InkUniform::default();
            let (row_x, row_y) = rows(inverse);
            ink.row_x = row_x;
            ink.row_y = row_y;
            let mut texture = cache.white.clone();
            let mut color = [1.0; 4];
            let rect = match &item.primitive {
                CanvasPrimitive::Rect {
                    rect,
                    color: rgba,
                    radius,
                } => {
                    ink.parameters = Vec4::new(0.0, *radius, 0.0, 0.0);
                    color = *rgba;
                    *rect
                }
                CanvasPrimitive::Ellipse { rect, color: rgba } => {
                    ink.parameters.x = 1.0;
                    color = *rgba;
                    *rect
                }
                CanvasPrimitive::Line {
                    points,
                    color: rgba,
                    width,
                } => {
                    ink.parameters = Vec4::new(2.0, 0.0, *width, points.len() as f32);
                    color = *rgba;
                    for (dest, point) in ink.points.iter_mut().zip(points) {
                        *dest = Vec4::new(point[0], point[1], 0.0, 0.0);
                    }
                    points_bounds(points, *width * 0.5)
                }
                CanvasPrimitive::Polygon {
                    points,
                    color: rgba,
                } => {
                    ink.parameters = Vec4::new(3.0, 0.0, 0.0, points.len() as f32);
                    color = *rgba;
                    for (dest, point) in ink.points.iter_mut().zip(points) {
                        *dest = Vec4::new(point[0], point[1], 0.0, 0.0);
                    }
                    points_bounds(points, 0.0)
                }
                CanvasPrimitive::Image { asset, rect } => {
                    ink.parameters.x = 4.0;
                    texture = cache.images[asset].clone();
                    *rect
                }
                CanvasPrimitive::Text {
                    text,
                    position,
                    color: rgba,
                    size,
                } => {
                    ink.parameters.x = 4.0;
                    color = *rgba;
                    let key = text_key(surface, text, *size, quality);
                    let mask = if let Some(mask) = cache.text.get(&key) {
                        mask.clone()
                    } else {
                        let (image, rect) = match text_mask(
                            fonts.get(&surface.font).unwrap(),
                            text,
                            *size,
                            quality,
                        ) {
                            Ok(value) => value,
                            Err(problem) => {
                                resource_problem(surface, &problem, &mut error, &mut next);
                                return;
                            }
                        };
                        let pixels = image.width() as usize * image.height() as usize;
                        if cache.text.values().map(|mask| mask.pixels).sum::<usize>() + pixels
                            > MAX_TEXT_PIXELS
                        {
                            resource_problem(surface,"Active canvas text masks exceed the portable 16-megapixel cache budget",&mut error,&mut next);
                            return;
                        }
                        let mask = TextMask {
                            image: images.add(image),
                            rect,
                            pixels,
                        };
                        cache.text.insert(key, mask.clone());
                        mask
                    };
                    texture = mask.image;
                    [
                        position[0] + mask.rect[0],
                        position[1] + mask.rect[1],
                        mask.rect[2],
                        mask.rect[3],
                    ]
                }
                CanvasPrimitive::Group { .. } | CanvasPrimitive::Hit { .. } => {
                    unreachable!("flattened drawing leaves")
                }
            };
            if rect[2] <= 0.0 || rect[3] <= 0.0 || color[3] <= 0.0 {
                continue;
            }
            ink.rect = Vec4::from_array(rect);
            // Expand by a reference pixel for antialiasing, then constrain the
            // quad to the canvas and each transformed clip's conservative AABB.
            let mut bounds = transformed_bounds(item.matrix, rect);
            bounds[0] -= 1.0;
            bounds[1] -= 1.0;
            bounds[2] += 2.0;
            bounds[3] += 2.0;
            bounds = intersect_bounds(bounds, [0.0, 0.0, surface.extent.x, surface.extent.y]);
            for clip in &item.clips {
                bounds = intersect_bounds(bounds, transformed_bounds(clip.matrix, clip.rect));
            }
            if bounds[2] <= 0.0 || bounds[3] <= 0.0 {
                continue;
            }
            ink.bounds = Vec4::from_array(bounds);
            color[3] *= item.opacity;
            let authored = color;
            let mut tinted = color;
            tinted[3] *= surface.opacity;
            ink.color = Color::srgba(tinted[0], tinted[1], tinted[2], tinted[3])
                .to_linear()
                .to_vec4();
            let key = (surface.screen.clone(), surface.element.clone(), index);
            let handle = if let Some(handle) = cache.materials.get(&key).cloned() {
                if let Some(material) = materials.get_mut(&handle) {
                    material.ink = ink;
                    material.texture = texture;
                }
                handle
            } else {
                let handle = materials.add(CanvasMaterial { ink, texture });
                cache.materials.insert(key, handle.clone());
                handle
            };
            prepared.push((item, authored, bounds, handle));
        }
        for (item, original_color, bounds, material) in prepared {
            let leaf = commands
                .spawn((
                    CanvasLeaf {
                        surface: entity,
                        item,
                        original_color,
                    },
                    InkTint {
                        original: [1.0, 1.0, 1.0, surface.opacity],
                        current: [1.0, 1.0, 1.0, surface.opacity],
                    },
                    MaterialNodeBundle {
                        // Drawing coordinates and pointer hits start at the outer
                        // border; Taffy positions absolute children from the inner one.
                        style: Style {
                            position_type: PositionType::Absolute,
                            left: Val::Px(
                                bounds[0] * factor.x - pixel_border(host_style.border.left),
                            ),
                            top: Val::Px(
                                bounds[1] * factor.y - pixel_border(host_style.border.top),
                            ),
                            width: Val::Px(bounds[2] * factor.x),
                            height: Val::Px(bounds[3] * factor.y),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        material,
                        focus_policy: FocusPolicy::Pass,
                        ..default()
                    },
                ))
                .id();
            commands.entity(entity).add_child(leaf);
        }
        commands.entity(entity).insert(Built);
    }
}

fn pixel_border(value: Val) -> f32 {
    if let Val::Px(pixels) = value {
        pixels
    } else {
        0.0
    }
}

/// Material clipping is evaluated after layout, scrolling and animated affine
/// transforms. Bevy's axis-aligned CPU clip is removed only from these quads;
/// every actual ancestor clip is instead intersected in its own local space.
fn sync_materials(
    mut commands: Commands,
    leaves: Query<(Entity, &CanvasLeaf, &InkTint, &Handle<CanvasMaterial>)>,
    surfaces: Query<(&CanvasSurface, &Node, &GlobalTransform)>,
    ancestors: Query<(&Node, &GlobalTransform, &Style, Option<&Parent>)>,
    mut materials: ResMut<Assets<CanvasMaterial>>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
) {
    for (entity, leaf, tint, handle) in &leaves {
        let Ok((surface, node, transform)) = surfaces.get(leaf.surface) else {
            continue;
        };
        if !node.size().cmpgt(Vec2::ZERO).all() {
            continue;
        }
        if !materials.contains(handle) {
            continue;
        }
        let mut clips = Vec::new();
        clips.push(ClipUniform {
            row_x: Vec4::new(1.0, 0.0, 0.0, 0.0),
            row_y: Vec4::new(0.0, 1.0, 0.0, 0.0),
            rect: Vec4::new(0.0, 0.0, surface.extent.x, surface.extent.y),
        });
        for clip in &leaf.item.clips {
            let Some(clip) = clip_uniform(clip) else {
                continue;
            };
            clips.push(clip);
        }
        let canvas_world = transform.compute_matrix()
            * Mat4::from_scale((node.size() / surface.extent).extend(1.0))
            * Mat4::from_translation((-surface.extent * 0.5).extend(0.0));
        let mut ancestor = leaf.surface;
        while let Ok((node, transform, style, parent)) = ancestors.get(ancestor) {
            if style.overflow.x != OverflowAxis::Visible
                || style.overflow.y != OverflowAxis::Visible
            {
                let matrix = transform.compute_matrix();
                if matrix.determinant().abs() < 1.0e-8 {
                    clips.push(ClipUniform::default());
                } else {
                    let (row_x, row_y) = rows(affine(matrix.inverse() * canvas_world));
                    let extent = node.size();
                    let low = -extent * 0.5;
                    let rect = Vec4::new(
                        if style.overflow.x == OverflowAxis::Visible {
                            -1_000_000.0
                        } else {
                            low.x
                        },
                        if style.overflow.y == OverflowAxis::Visible {
                            -1_000_000.0
                        } else {
                            low.y
                        },
                        if style.overflow.x == OverflowAxis::Visible {
                            2_000_000.0
                        } else {
                            extent.x
                        },
                        if style.overflow.y == OverflowAxis::Visible {
                            2_000_000.0
                        } else {
                            extent.y
                        },
                    );
                    clips.push(ClipUniform { row_x, row_y, rect });
                }
            }
            let Some(parent) = parent else {
                break;
            };
            ancestor = parent.get();
        }
        if clips.len() > MAX_CLIPS {
            resource_problem(
                surface,
                "Canvas/ancestor clipping exceeds the portable 64-clip renderer budget",
                &mut error,
                &mut next,
            );
            return;
        }
        let mut clipping = [ClipUniform::default(); MAX_CLIPS];
        clipping[..clips.len()].copy_from_slice(&clips);
        let base = leaf.original_color;
        let color = Color::srgba(
            base[0] * tint.current[0],
            base[1] * tint.current[1],
            base[2] * tint.current[2],
            base[3] * tint.current[3],
        )
        .to_linear()
        .to_vec4();
        let changed = materials.get(handle).is_some_and(|material| {
            material.ink.clips != clipping
                || material.ink.counts.x != clips.len() as f32
                || material.ink.color != color
        });
        if changed {
            let material = materials.get_mut(handle).unwrap();
            material.ink.clips = clipping;
            material.ink.counts.x = clips.len() as f32;
            material.ink.color = color;
        }
        commands.entity(entity).remove::<bevy::ui::CalculatedClip>();
    }
}

pub(crate) fn reset_tints(world: &mut World) {
    for mut tint in world.query::<&mut InkTint>().iter_mut(world) {
        tint.current = tint.original;
    }
}
pub(crate) fn tint(world: &mut World, entity: Entity, pose: &rvn_ui::motion::Pose) {
    if let Some(mut tint) = world.get_mut::<InkTint>(entity) {
        tint.current[0] *= pose.tint_r;
        tint.current[1] *= pose.tint_g;
        tint.current[2] *= pose.tint_b;
        tint.current[3] *= pose.tint_a * pose.opacity;
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "qa_custom_canvas.rs"]
mod qa;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn install_qa(app: &mut App) {
    app.add_systems(PostUpdate, qa::drive.after(sync_materials));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focused_canvases_do_not_swallow_game_save_load_and_menu_shortcuts() {
        for key in [KeyCode::Tab, KeyCode::Escape, KeyCode::F5, KeyCode::F6] {
            assert!(reserved_game_key(key));
        }
        for key in [
            KeyCode::ArrowLeft,
            KeyCode::Enter,
            KeyCode::Space,
            KeyCode::KeyA,
            KeyCode::F4,
        ] {
            assert!(!reserved_game_key(key));
        }
    }
    #[test]
    fn inverse_and_clip_uniforms_match_shared_hit_geometry() {
        let matrix = rvn_ui::custom_canvas::group_matrix([20.0, 30.0, 2.0, 0.5, 37.0, 1.0]);
        let local = [4.0, 7.0];
        let global = transform_point(matrix, local);
        let inverse = inverse_matrix(matrix).unwrap();
        let restored = transform_point(inverse, global);
        assert!((restored[0] - local[0]).abs() < 1e-4 && (restored[1] - local[1]).abs() < 1e-4);
        let clip = clip_uniform(&CanvasClip {
            rect: [0.0, 0.0, 10.0, 10.0],
            matrix,
        })
        .unwrap();
        let p = Vec3::new(global[0], global[1], 1.0);
        assert!((clip.row_x.truncate().dot(p) - 4.0).abs() < 1e-4);
        assert!(inverse_matrix([0.0; 6]).is_none());
    }
    #[test]
    fn analytic_shapes_bound_their_entire_transformed_geometry() {
        assert_eq!(
            points_bounds(&[[0.0, 10.0], [20.0, -10.0]], 2.0),
            [-2.0, -12.0, 24.0, 24.0]
        );
        assert_eq!(
            intersect_bounds([-2.0, -3.0, 10.0, 10.0], [0.0, 0.0, 5.0, 5.0]),
            [0.0, 0.0, 5.0, 5.0]
        );
        let bounds = transformed_bounds([0.0, 1.0, -1.0, 0.0, 20.0, 30.0], [0.0, 0.0, 10.0, 20.0]);
        assert_eq!(bounds, [0.0, 30.0, 20.0, 10.0]);
        assert_eq!(pixel_border(Val::Px(1.5)), 1.5);
        assert_eq!(pixel_border(Val::Auto), 0.0);
        let outer_left = 42.0;
        let inner_border = 1.5;
        assert_eq!(
            outer_left - pixel_border(Val::Px(inner_border)) + inner_border,
            outer_left
        );
    }
    #[test]
    fn motion_tints_restore_without_drifting_or_touching_saved_state() {
        let mut world = World::new();
        let entity = world
            .spawn(InkTint {
                original: [1.0, 1.0, 1.0, 0.8],
                current: [1.0, 1.0, 1.0, 0.8],
            })
            .id();
        tint(
            &mut world,
            entity,
            &rvn_ui::motion::Pose {
                tint_r: 0.5,
                opacity: 0.25,
                ..default()
            },
        );
        assert_eq!(
            world.get::<InkTint>(entity).unwrap().current,
            [0.5, 1.0, 1.0, 0.2]
        );
        reset_tints(&mut world);
        assert_eq!(
            world.get::<InkTint>(entity).unwrap().current,
            [1.0, 1.0, 1.0, 0.8]
        );
    }
    #[test]
    fn shader_layout_stays_within_webgl2_minimum_uniform_budget() {
        use bevy::render::render_resource::encase::ShaderType;
        assert!(InkUniform::min_size().get() <= 16_384);
        let shader = include_str!("custom_canvas.wgsl");
        assert!(shader.contains("array<vec4<f32>, 256>"));
        assert!(shader.contains("array<Clip, 64>"));
        assert!(!shader.contains("var<storage>"));
    }
    #[test]
    fn bundled_unicode_font_masks_share_kerning_and_line_metrics() {
        let font =
            Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap();
        let (one, bounds) = text_mask(&font, "AV éΩ", 24.0, 1.0).unwrap();
        let (same, same_bounds) = text_mask(&font, "AV éΩ", 24.0, 1.0).unwrap();
        assert_eq!(one.data, same.data);
        assert_eq!(bounds, same_bounds);
        assert!(one.data.chunks_exact(4).any(|pixel| pixel[3] > 0));
        let (_, two_lines) = text_mask(&font, "AV éΩ\nAV éΩ", 24.0, 1.0).unwrap();
        assert!(two_lines[3] > bounds[3] + 12.0);
        let (sharp, sharp_bounds) = text_mask(&font, "AV éΩ", 24.0, 2.0).unwrap();
        assert!(sharp.width() > one.width());
        assert!((sharp_bounds[2] - bounds[2]).abs() < 2.0);
        let (_, kerning) = text_mask(&font, "AV", 24.0, 1.0).unwrap();
        let (_, separated) = text_mask(&font, "A V", 24.0, 1.0).unwrap();
        assert!(kerning[2] < separated[2]);
        assert!(text_mask(&font, "W", 16_384.0, 4.0).is_err());
        let combining = "\u{0301}".repeat(32_768);
        let problem = text_mask(&font, &combining, 512.0, 1.0).unwrap_err();
        assert!(problem.contains("64-megapixel glyph-rasterization"));
    }
}
