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
use rvn_core::ui_input::{UiInputSource, ScopedUiInput};
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

#[derive(Resource,Default)]
pub(crate) struct DebuggerHostProducer;
pub(crate) struct CustomCanvasPlugin;
impl Plugin for CustomCanvasPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebuggerHostProducer>();
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
            .init_resource::<CanvasPointerProducers>()
            .add_systems(PreUpdate, capture_pointer_producers.after(crate::programmable_ui::UiInputCaptureSet).after(bevy::input::InputSystem))
            .add_systems(
                Update,
                ((
                    tick.run_if(crate::scene_publication::host_frame_ready).in_set(crate::scene_publication::CanvasFeedbackSet)
                        .before(crate::programmable_ui::InterfaceInputSet),
                    input.run_if(crate::runtime_restart::input_available).run_if(resource_exists::<VnEngine>.and_then(crate::scene_publication::ready))
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
                )).distributive_run_if(crate::runtime_restart::presentation_available),
            )
            .add_systems(
                PostUpdate,
                (sync_materials
                    .after(bevy::ui::UiSystem::Layout)
                    .after(bevy::ui::UiSystem::Stack)
                    .after(bevy::transform::TransformSystem::TransformPropagate)
                    .after(bevy::ui::update::update_clipping_system)
                    .after(crate::programmable_ui::InterfaceScrollSet)).distributive_run_if(crate::runtime_restart::presentation_available),
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

// PRIVATE NOT_RUN. Existing CanvasFeedbackSet producer, no second clock.
pub(crate) fn tick(world:&mut World) {
    // Startup keeps accepted canvas presentation; this clock mutates only Game.
    if !world.contains_resource::<VnEngine>() { return; }
    if world.get_resource::<VnEngine>().is_some_and(|engine|engine.0.state.scene_execution.is_some()) {
        crate::scene_publication::host_frame(world);
    } else {
        // The old legacy producer/gates and delta are retained, not a second
        // modern route. A real exclusive World system preserves tick's set.
        use bevy::ecs::system::RunSystemOnce;
        world.run_system_once(tick_legacy);
    }
}
fn tick_legacy(
    time: Res<Time>,
    state: Res<State<VnState>>,
    windows: Query<(&Window, Option<&bevy::window::PrimaryWindow>)>,
    accessibility: Res<crate::accessibility::Accessibility>,
    mut engine: ResMut<VnEngine>,
    mut screens: ResMut<Screens>,
    mut output: EventWriter<VnCommand>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
    source: Option<Res<crate::source_menus::SourceMenus>>,
) {
    let opted = engine.0.state.scene_execution.is_some();
    if opted {
        let focused = windows.iter().find_map(|(window, primary)|
            primary.is_some().then_some(window.focused))
            .unwrap_or_else(|| windows.iter().all(|(window, _)| window.focused));
        if !crate::scene_publication::host_clock_allowed(&engine, focused, accessibility.blocked) {
            return;
        }
    } else {
        // Preserve the legacy state/menu/focus/Canvas gate and f32 delta route.
        if engine.0.renderer.scene_frame_failed || accessibility.blocked
            || (!source.as_deref().is_some_and(|source| source.any()) && !matches!(
                state.get(), VnState::Waiting | VnState::Stepping | VnState::Animating)) {
            return;
        }
        if windows.get_single().is_ok_and(|(window, _)| !window.focused) { return; }
        let mut canvas = false;
        for view in &screens.views {
            view.root.visit(&mut |component| canvas |= component.kind == ComponentKind::Canvas);
        }
        if !canvas { return; }
    }
    let result = if opted {
        engine.0.interface_tick_host(time.delta_seconds_f64().min(0.25))
    } else {
        engine.0.interface_tick(f64::from(time.delta_seconds()).min(0.25))
    };
    if let Err(problem) = result {
        // A refused automatic opted Host instant cannot retry its handlers on
        // every subsequent frame. Explicit existing recovery inputs stay live.
        if opted { engine.0.renderer.scene_frame_failed = true; }
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

#[derive(Resource, Default)]
struct CanvasInputSources(std::collections::HashMap<(String,u64), UiInputSource>);

// A raw event retains its producer's physical frame, including an empty hit.
// The shared Arc is transient; it is neither saved nor a second UI authority.
#[derive(Clone)]
struct CanvasPhysicalGeometry {
    inverse: Mat4,
    z: f32,
    size: Vec2,
    extent: Vec2,
    clips: Vec<(Mat4, f32, Vec2, [bool; 2])>,
}
impl CanvasPhysicalGeometry {
    fn local(&self, point: Vec2, captured: bool) -> Option<Vec2> {
        let local = self.inverse.transform_point3(point.extend(self.z)).truncate();
        if !local.is_finite() || !self.size.cmpgt(Vec2::ZERO).all() { return None; }
        if !captured {
            if !local.abs().cmple(self.size * 0.5).all() { return None; }
            for (inverse, z, size, clip) in &self.clips {
                let local = inverse.transform_point3(point.extend(*z)).truncate();
                if !local.is_finite()
                    || (clip[0] && local.x.abs() > size.x * 0.5)
                    || (clip[1] && local.y.abs() > size.y * 0.5) { return None; }
            }
        }
        let local = (local / self.size + Vec2::splat(0.5)) * self.extent;
        local.is_finite().then_some(local)
    }
}
#[derive(Clone)]
struct CanvasPhysicalTarget {
    capture: Capture,
    source: UiInputSource,
    geometry: CanvasPhysicalGeometry,
    drawing: CanvasDrawing,
}
struct CanvasPhysicalNode {
    inverse: Mat4,
    z: f32,
    size: Vec2,
    blocking: bool,
    target: Option<CanvasPhysicalTarget>,
}
#[derive(Default)]
struct CanvasPhysicalFrame {
    nodes: Vec<CanvasPhysicalNode>,
}
impl CanvasPhysicalFrame {
    fn pick(&self, point: Vec2) -> Option<&CanvasPhysicalTarget> {
        for node in self.nodes.iter().rev() {
            let local = node.inverse.transform_point3(point.extend(node.z)).truncate();
            if !local.is_finite() || !local.abs().cmple(node.size * 0.5).all() { continue; }
            if let Some(target) = &node.target {
                target.geometry.local(point, false)?;
                return Some(target);
            }
            if node.blocking { return None; }
        }
        None
    }
    fn target(&self, capture: &Capture) -> Option<&CanvasPhysicalTarget> {
        self.nodes.iter().filter_map(|node|node.target.as_ref()).find(|target|
            target.capture.screen == capture.screen && target.capture.element == capture.element
                && target.capture.order == capture.order && target.capture.resource_token == capture.resource_token)
    }
}
fn physical_geometry(world: &World, entity: Entity, extent: Vec2) -> Option<CanvasPhysicalGeometry> {
    let node = world.get::<Node>(entity)?;
    let transform = world.get::<GlobalTransform>(entity)?;
    let matrix = transform.compute_matrix();
    if !matrix.is_finite() || matrix.determinant().abs() < 1e-8 || !node.size().cmpgt(Vec2::ZERO).all() { return None; }
    let mut clips = Vec::new();
    let mut ancestor = entity;
    loop {
        let style = world.get::<Style>(ancestor)?;
        let transform = world.get::<GlobalTransform>(ancestor)?;
        let node = world.get::<Node>(ancestor)?;
        let matrix = transform.compute_matrix();
        if !matrix.is_finite() || matrix.determinant().abs() < 1e-8 { return None; }
        clips.push((matrix.inverse(), transform.translation().z, node.size(),
            [style.overflow.x != OverflowAxis::Visible, style.overflow.y != OverflowAxis::Visible]));
        let Some(parent) = world.get::<Parent>(ancestor) else { break; };
        ancestor = parent.get();
    }
    Some(CanvasPhysicalGeometry { inverse: matrix.inverse(), z: transform.translation().z,
        size: node.size(), extent, clips })
}
fn capture_physical_frame(world: &World) -> CanvasPhysicalFrame {
    let mut frame = CanvasPhysicalFrame::default();
    let (Some(stack), Some(screens), Some(engine)) = (world.get_resource::<bevy::ui::UiStack>(),
        world.get_resource::<Screens>(), world.get_resource::<VnEngine>()) else { return frame; };
    for &entity in &stack.uinodes {
        if world.get::<ViewVisibility>(entity).is_some_and(|visible|!visible.get()) { continue; }
        let (Some(node), Some(transform)) = (world.get::<Node>(entity),world.get::<GlobalTransform>(entity)) else { continue; };
        let matrix = transform.compute_matrix();
        if !matrix.is_finite() || matrix.determinant().abs() < 1e-8 { continue; }
        let surface = world.get::<CanvasSurface>(entity);
        let target = surface.and_then(|surface| {
            let view = screens.views.iter().find(|view|view.name == surface.screen)?;
            let source = world.get::<crate::programmable_ui::ControlSource>(entity).map(|source|source.0.clone())
                .or_else(||engine.0.state.scene_execution.is_none().then(||engine.0.capture_ui_input_source(&surface.screen)).flatten())?;
            if source.order != view.order || !engine.0.ui_input_source_current(&source) { return None; }
            let resource_token = crate::programmable_ui::rendered_canvas_input_token(screens,&surface.screen,&surface.element);
            if engine.0.state.scene_execution.is_some() && resource_token.is_none() { return None; }
            let capture = Capture { screen: surface.screen.clone(), element:surface.element.clone(), order:view.order,
                button:String::new(), position:Vec2::ZERO, resource_token };
            if !target_available(world,&capture) { return None; }
            Some(CanvasPhysicalTarget { capture, source, geometry:physical_geometry(world,entity,surface.extent)?,
                drawing:surface.drawing.clone() })
        });
        frame.nodes.push(CanvasPhysicalNode { inverse:matrix.inverse(),z:transform.translation().z,size:node.size(),
            blocking:surface.is_some() || world.get::<crate::programmable_ui::Control>(entity).is_some()
                || world.get::<FocusPolicy>(entity)==Some(&FocusPolicy::Block), target });
    }
    frame
}
#[derive(Clone)]
struct CanvasProducedPointer {
    point: Option<Vec2>,
    frame: std::sync::Arc<CanvasPhysicalFrame>,
}
#[derive(Resource, Default)]
struct CanvasPointerProducers {
    mouse_reader: ManualEventReader<MouseButtonInput>,
    motion_reader: ManualEventReader<CursorMoved>,
    touch_reader: ManualEventReader<TouchInput>,
    wheel_reader: ManualEventReader<MouseWheel>,
    mouse: HashMap<usize,CanvasProducedPointer>,
    motion: HashMap<usize,CanvasProducedPointer>,
    touch: HashMap<usize,CanvasProducedPointer>,
    wheel: HashMap<usize,CanvasProducedPointer>,
}
fn retain_pointer_events<E: bevy::ecs::event::Event>(world:&World, reader:&mut ManualEventReader<E>,
    produced:&mut HashMap<usize,CanvasProducedPointer>,frame:&std::sync::Arc<CanvasPhysicalFrame>,
    point:impl Fn(&E)->Option<Vec2>) {
    let Some(events)=world.get_resource::<Events<E>>() else { return; };
    produced.retain(|id,_|*id>=events.oldest_event_count());
    for (event,id) in reader.read_with_id(events) {
        produced.entry(id.id).or_insert_with(||CanvasProducedPointer { point:point(event),frame:frame.clone() });
    }
}
#[derive(Clone)]
pub(crate) struct PreviewCanvasFrame(std::sync::Arc<CanvasPhysicalFrame>);
pub(crate) fn preview_canvas_cost(world:&World)->Option<usize> {
    let Some(stack)=world.get_resource::<bevy::ui::UiStack>()else{return Some(0);};
    if stack.uinodes.len()>4096{return None;}
    let mut cost=crate::editor_preview_input::PreviewCost::new(1024*1024);
    cost.add(stack.uinodes.len().checked_mul(512)?)?;
    for &entity in &stack.uinodes {
        if let Some(surface)=world.get::<CanvasSurface>(entity) {
            cost.add(surface.screen.len().checked_add(surface.element.len())?)?;
            if let Some(source)=world.get::<crate::programmable_ui::ControlSource>(entity) {cost.add(crate::editor_preview_input::preview_source_cost(&source.0)?)?;}
            else {cost.add(surface.screen.len().checked_mul(6)?.checked_add(512)?)?;}
            if let Some(screens)=world.get_resource::<Screens>() {
                if let Some(token)=crate::programmable_ui::borrowed_canvas_input_token(screens,&surface.screen,&surface.element) {cost.add(crate::editor_preview_input::preview_token_cost(token)?)?;}
            }
            // Traverse borrowed ancestors before physical_geometry allocates its
            // clip vector. The budget also bounds malformed cycles/deep trees.
            let mut ancestor=entity;
            loop {cost.add(256)?;let Some(parent)=world.get::<Parent>(ancestor)else{break;};ancestor=parent.get();}
            surface.drawing.validate().ok()?;
            cost.serialized(&surface.drawing)?;
        }
    }
    Some(cost.bytes)
}
pub(crate) fn capture_preview_canvas(world:&World)->Option<(PreviewCanvasFrame,usize)> {
    let cost=preview_canvas_cost(world)?;
    let frame=capture_physical_frame(world);
    Some((PreviewCanvasFrame(std::sync::Arc::new(frame)),cost))
}
impl PreviewCanvasFrame {
    pub(crate) fn mouse(&self,world:&mut World,event:MouseButtonInput,point:Vec2){
        let id=world.resource_mut::<Events<MouseButtonInput>>().send(event).id;
        world.init_resource::<CanvasPointerProducers>();
        world.resource_mut::<CanvasPointerProducers>().mouse.insert(id,CanvasProducedPointer{point:Some(point),frame:self.0.clone()});
    }
    pub(crate) fn motion(&self,world:&mut World,event:CursorMoved){
        let point=event.position;
        let id=world.resource_mut::<Events<CursorMoved>>().send(event).id;
        world.init_resource::<CanvasPointerProducers>();
        world.resource_mut::<CanvasPointerProducers>().motion.insert(id,CanvasProducedPointer{point:Some(point),frame:self.0.clone()});
    }
    pub(crate) fn wheel(&self,world:&mut World,event:MouseWheel,point:Vec2){
        let id=world.resource_mut::<Events<MouseWheel>>().send(event).id;
        world.init_resource::<CanvasPointerProducers>();
        world.resource_mut::<CanvasPointerProducers>().wheel.insert(id,CanvasProducedPointer{point:Some(point),frame:self.0.clone()});
    }
}
fn capture_pointer_producers(world:&mut World) {
    if !crate::runtime_restart::input_available(world){cancel_runtime_input(world); }
    let mut produced=world.remove_resource::<CanvasPointerProducers>().unwrap_or_default();
    let frame=std::sync::Arc::new(if crate::runtime_restart::input_available(world){capture_physical_frame(world)}else{CanvasPhysicalFrame::default()});
    let cursor=world.query::<&Window>().iter(world).next().and_then(Window::cursor_position);
    retain_pointer_events(world,&mut produced.mouse_reader,&mut produced.mouse,&frame,|_|cursor);
    retain_pointer_events(world,&mut produced.motion_reader,&mut produced.motion,&frame,|event|Some(event.position));
    retain_pointer_events(world,&mut produced.touch_reader,&mut produced.touch,&frame,|event|Some(event.position));
    retain_pointer_events(world,&mut produced.wheel_reader,&mut produced.wheel,&frame,|_|cursor);
    world.insert_resource(produced);
}

#[derive(Clone, Debug)]
struct Capture {
    screen: String,
    element: String,
    order: u64,
    button: String,
    position: Vec2,
    resource_token: Option<rvn_core::scene_resources::ResourceToken>,
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

/// Retire physical captures while retaining the last real modifier state.
/// Suspended PreUpdate refreshes this baseline after InputSystem; a later
/// genuine K then CtrlRelease must still see Ctrl at K's ordered keydown.
pub(crate) fn cancel_runtime_input(world:&mut World){
    cancel_preview_pointer_capture(world,true);
    let modifiers=world.get_resource::<ButtonInput<KeyCode>>().map(|held|[
        held.pressed(KeyCode::ControlLeft),held.pressed(KeyCode::ControlRight),
        held.pressed(KeyCode::ShiftLeft),held.pressed(KeyCode::ShiftRight),
        held.pressed(KeyCode::AltLeft),held.pressed(KeyCode::AltRight),
        held.pressed(KeyCode::SuperLeft),held.pressed(KeyCode::SuperRight),
    ]).unwrap_or([false;8]);
    if let Some(mut pointer)=world.get_resource_mut::<PointerState>(){pointer.modifiers=modifiers;}
}
pub(crate) fn cancel_preview_pointer_capture(world:&mut World,all:bool){
    if let Some(mut pointer)=world.get_resource_mut::<PointerState>() {
        if all {pointer.captures.clear();pointer.suppressed.clear();pointer.modifiers=[false;8];}
        else {pointer.captures.remove("mouse");pointer.suppressed.remove("mouse");}
    }
    // Preserve ordinals as negative captures, so PreUpdate cannot give an old
    // deferred edge new geometry or a new source after cancellation.
    if let Some(mut producers)=world.get_resource_mut::<CanvasPointerProducers>() {
        let empty=std::sync::Arc::new(CanvasPhysicalFrame::default());
        for produced in producers.mouse.values_mut(){produced.point=None;produced.frame=empty.clone();}
        for produced in producers.motion.values_mut(){produced.point=None;produced.frame=empty.clone();}
        if all {
            for produced in producers.wheel.values_mut(){produced.point=None;produced.frame=empty.clone();}
            for produced in producers.touch.values_mut(){produced.point=None;produced.frame=empty.clone();}
        }
    }
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
fn capture_matches_live_instance(world:&World,capture:&Capture)->bool {
    if let Some(token)=&capture.resource_token {
        let game=&world.resource::<VnEngine>().0;
        if !game.state.scene_execution.as_ref().is_some_and(|execution|execution.resources.accepts(token,game.runtime_epoch(),execution.owner())) {return false;}
    }
    world.resource::<Screens>().views.iter().any(|view|view.name==capture.screen && view.order==capture.order)
}
fn target_available(world: &World, capture: &Capture) -> bool {
    if !capture_matches_live_instance(world,capture) {return false;}
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
                resource_token: world.resource::<VnEngine>().0.scene_resource_token(rvn_core::scene_resources::ResourceTaskKind::Canvas,&format!("{}/{}",surface.screen,surface.element)),
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
fn dispatch(world: &mut World, source: UiInputSource, event: UiInput) -> bool {
    if world.resource::<VnEngine>().0.renderer.menu_pending || world.resource::<VnEngine>().0.renderer.scene_batch_pending() {return false;}
    let before_epoch=world.resource::<VnEngine>().0.runtime_epoch();
    let before_owner=world.resource::<VnEngine>().0.state.scene_execution.as_ref().and_then(|execution|execution.owner());
    let source_event=world.resource::<VnEngine>().0.state.ui.screens.iter().any(|screen|screen.name==event.screen && screen.host_role.is_some());
    if world.resource::<VnEngine>().0.renderer.scene_frame_failed && !source_event {return false;}
    if crate::editor_debugger::enabled(world.resource::<VnEngine>()){
        return match crate::editor_debugger::input(world.resource::<VnEngine>(),ScopedUiInput{source,event}){
            Ok(accepted)=>accepted,
            Err(problem)=>{world.resource_mut::<ScriptErrorMessage>().0=problem;if !source_event{world.resource_mut::<NextState<VnState>>().set(VnState::Error);}false}
        };
    }
    match world.resource_mut::<VnEngine>().0.interface_event_scoped(ScopedUiInput { source: source.clone(), event }) {
        Ok(false) => return false,
        Ok(true) => {},
        Err(problem) => {
        world.resource_mut::<ScriptErrorMessage>().0 =
            rvn_core::error::ScriptError::from_runtime(&problem).to_string();
        if source_event||problem.is_menu_request_rejection() {warn!("Game menu canvas event: {}",world.resource::<ScriptErrorMessage>().0);}
        else {world.resource_mut::<NextState<VnState>>().set(VnState::Error);}
        return false;
        }
    }
    crate::scene_publication::consume(world);
    let game=&world.resource::<VnEngine>().0;
    let still_current=game.ui_input_source_current(&source);
    if game.runtime_epoch()!=before_epoch || game.state.scene_execution.as_ref().and_then(|execution|execution.owner())!=before_owner {return false;}
    for event in world.resource_mut::<VnEngine>().0.renderer.take_pending() {
        if let VnCommand::Interfaces(views) = &event {
            crate::programmable_ui::update_views(
                &mut world.resource_mut::<Screens>(),
                views.clone(),
            );
        }
        world.send_event(event);
    }
    still_current
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
    deliver_from_frame(world,capture,pointer,kind,point,captured,modifiers,wheel,None)
}
fn deliver_from_frame(
    world: &mut World,
    capture: &Capture,
    pointer: &str,
    kind: ScreenEventKind,
    point: Vec2,
    captured: bool,
    modifiers: [bool; 8],
    wheel: Vec2,
    physical: Option<&CanvasPhysicalFrame>,
) -> bool {
    if !capture_matches_live_instance(world,capture) {return true;}
    let Some(source) = world.get_resource::<CanvasInputSources>()
        .and_then(|sources|sources.0.get(&(capture.screen.clone(),capture.order))).cloned()
        .filter(|source|world.resource::<VnEngine>().0.ui_input_source_current(source)) else { return true; };
    let (local, frozen_drawing) = if let Some(physical) = physical {
        let Some(target) = physical.target(capture).filter(|target|target.source==source) else { return true; };
        let Some(local) = target.geometry.local(point,captured) else { return true; };
        (local, Some(&target.drawing))
    } else {
        let Some(entity) = canvas_entity(world, &capture.screen, &capture.element) else { return true; };
        let Some(local) = local_position(world,entity,point,captured) else { return true; };
        (local,None)
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
    let hit = frozen_drawing.or(component.drawing.as_ref())
        .and_then(|drawing| flatten(drawing).ok())
        .and_then(|items| rvn_ui::custom_canvas::hit_test(&items, local.to_array()));
    if component.consume_input {
        world.resource_mut::<Screens>().pointer_consumed = true;
    }
    dispatch(
        world,
        source,
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
    route_pointer_from_frame(world,state,pointer,kind,point,button,wheel,None)
}
fn route_pointer_from_frame(
    world: &mut World,
    state: &mut PointerState,
    pointer: &str,
    kind: ScreenEventKind,
    point: Vec2,
    button: &str,
    wheel: Vec2,
    physical: Option<&CanvasPhysicalFrame>,
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
        let (mut capture, source) = if let Some(physical) = physical {
            let target=physical.pick(point)?;
            (target.capture.clone(),target.source.clone())
        } else {
            // Only existing standalone legacy adapters may synchronously pick.
            if crate::programmable_ui::frame_input_captured(world.resource::<Screens>()) || world.resource::<VnEngine>().0.state.scene_execution.is_some() { return None; }
            let (entity,capture,_) = pick(world,point)?;
            let source=world.get::<crate::programmable_ui::ControlSource>(entity).map(|source|source.0.clone())
                .or_else(||world.resource::<VnEngine>().0.capture_ui_input_source(&capture.screen))?;
            (capture,source)
        };
        let game=&world.resource::<VnEngine>().0;
        if source.order != capture.order || !game.ui_input_source_current(&source) || !target_available(world,&capture) { return None; }
        if crate::programmable_ui::frame_input_captured(world.resource::<Screens>())
            && crate::programmable_ui::frame_input_source(world.resource::<Screens>(),&capture.screen).as_ref()!=Some(&source) { return None; }
        world.init_resource::<CanvasInputSources>();
        // Bound live registry plus outstanding captures, never a name-only cache.
        let stale: Vec<_> = world.resource::<CanvasInputSources>().0.iter()
            .filter(|(_,source)|!world.resource::<VnEngine>().0.ui_input_source_current(source))
            .map(|(key,_)|key.clone()).collect();
        let mut sources=world.resource_mut::<CanvasInputSources>();
        for key in stale { sources.0.remove(&key); }
        sources.0.entry((capture.screen.clone(),capture.order)).or_insert(source);
        capture.button = button.into();
        Some(capture)
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
                let Some(source) = world.get_resource::<CanvasInputSources>()
                    .and_then(|sources|sources.0.get(&(capture.screen.clone(),capture.order))).cloned() else { return true; };
                if !dispatch(
                    world,
                    source,
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
    let mut success = deliver_from_frame(
        world,
        &capture,
        pointer,
        kind,
        point,
        captured,
        state.modifiers,
        wheel,
        physical,
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
        .read_with_id(world.resource::<Events<MouseButtonInput>>())
        .filter(|(_,id)|crate::runtime_restart::accepts_event::<MouseButtonInput>(world,id.id))
        .map(|(event,id)|((*event).clone(),id.id))
        .collect();
    let moves: Vec<_> = pointer
        .motion
        .read_with_id(world.resource::<Events<CursorMoved>>())
        .filter(|(_,id)|crate::runtime_restart::accepts_event::<CursorMoved>(world,id.id))
        .map(|(event,id)|((*event).clone(),id.id))
        .collect();
    let touches: Vec<_> = pointer
        .touch
        .read_with_id(world.resource::<Events<TouchInput>>())
        .filter(|(_,id)|crate::runtime_restart::accepts_event::<TouchInput>(world,id.id))
        .map(|(event,id)|((*event).clone(),id.id))
        .collect();
    let wheel: Vec<_> = pointer
        .wheel
        .read_with_id(world.resource::<Events<MouseWheel>>())
        .filter(|(_,id)|crate::runtime_restart::accepts_event::<MouseWheel>(world,id.id))
        .map(|(event,id)|((*event).clone(),id.id))
        .collect();
    let keys: Vec<_> = pointer
        .keyboard
        .read_with_id(world.resource::<Events<KeyboardInput>>())
        .map(|(event,id)|((*event).clone(),id.id))
        .collect();
    let lost_focus = pointer
        .focus
        .read(world.resource::<Events<WindowFocused>>())
        .any(|event| !event.focused);
    let key_target = crate::programmable_ui::frame_input_target(world.resource::<Screens>())
        .or_else(||(!crate::programmable_ui::frame_input_captured(world.resource::<Screens>()) && world.resource::<VnEngine>().0.state.scene_execution.is_none()).then(|| {
            let (screen,element)=world.resource::<Screens>().keyboard_focus.clone()?;
            let source=world.resource::<VnEngine>().0.capture_ui_input_source(&screen)?;
            Some((screen,element,source))
        }).flatten());
    reconcile_capture_authority(world,&mut pointer);
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
    if let Some((event,id)) = moves.last() {
        let produced=world.get_resource::<CanvasPointerProducers>().and_then(|p|p.motion.get(id)).cloned();
        success = route_pointer_from_frame(
            world,
            &mut pointer,
            "mouse",
            ScreenEventKind::PointerMove,
            produced.as_ref().and_then(|p|p.point).unwrap_or(event.position),
            "",
            Vec2::ZERO,
            produced.as_ref().map(|p|p.frame.as_ref()),
        );
    }
    if success {
        for (event,id) in mouse {
            let produced=world.get_resource::<CanvasPointerProducers>().and_then(|p|p.mouse.get(&id)).cloned();
            let position = produced.as_ref().and_then(|p|p.point).or_else(|| {
                if produced.is_some() { return None; }
                cursor
            }).or_else(|| {
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
            if !route_pointer_from_frame(
                world,
                &mut pointer,
                "mouse",
                kind,
                position,
                &button_name(event.button),
                Vec2::ZERO,
                produced.as_ref().map(|p|p.frame.as_ref()),
            ) {
                success = false;
                break;
            }
        }
    }
    if success {
        for (event,id) in touches {
            let produced=world.get_resource::<CanvasPointerProducers>().and_then(|p|p.touch.get(&id)).cloned();
            let kind = match event.phase {
                TouchPhase::Started => ScreenEventKind::PointerDown,
                TouchPhase::Moved => ScreenEventKind::PointerMove,
                TouchPhase::Ended => ScreenEventKind::PointerUp,
                TouchPhase::Canceled => ScreenEventKind::PointerCancel,
            };
            if !route_pointer_from_frame(
                world,
                &mut pointer,
                &format!("touch:{}", event.id),
                kind,
                produced.as_ref().and_then(|p|p.point).unwrap_or(event.position),
                "touch",
                Vec2::ZERO,
                produced.as_ref().map(|p|p.frame.as_ref()),
            ) {
                success = false;
                break;
            }
        }
    }
    if success {
        {
            for (event,id) in wheel {
                let produced=world.get_resource::<CanvasPointerProducers>().and_then(|p|p.wheel.get(&id)).cloned();
                let position=produced.as_ref().and_then(|p|p.point).or_else(||produced.is_none().then_some(cursor).flatten());
                let Some(position)=position else {continue;};
                let delta = Vec2::new(event.x, event.y)
                    * if event.unit == MouseScrollUnit::Line {
                        36.0
                    } else {
                        1.0
                    };
                if !route_pointer_from_frame(
                    world,
                    &mut pointer,
                    "mouse",
                    ScreenEventKind::Wheel,
                    position,
                    "",
                    delta,
                    produced.as_ref().map(|p|p.frame.as_ref()),
                ) {
                    success = false;
                    break;
                }
            }
        }
    }
    for (key,id) in keys {
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
        // Physical modifier state is still drained above, even for stale input.
        if !crate::runtime_restart::accepts_event::<KeyboardInput>(world,id)
            || !crate::programmable_ui::queued_native_key_current(world.resource::<Screens>(),world.resource::<VnEngine>(),id)
            || !success || reserved_game_key(key.key_code) {
            continue;
        }
        let Some((screen, element, source)) = key_target.clone() else {
            continue;
        };
        if !world.resource::<VnEngine>().0.ui_input_source_current(&source) { continue; }
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
            source,
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

fn reconcile_capture_authority(world:&World,pointer:&mut PointerState) {
    let epoch=world.resource::<VnEngine>().0.interface_epoch();
    if epoch!=pointer.epoch {
        let stale:Vec<_>=pointer.captures.iter().filter(|(_,capture)|capture.resource_token.is_none() || !target_available(world,capture)).map(|(id,_)|id.clone()).collect();
        for id in stale {pointer.captures.remove(&id);pointer.suppressed.insert(id);}
        pointer.epoch=epoch;
    }
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
    app.add_systems(PostUpdate, qa::drive.after(sync_materials).in_set(crate::qa_playthrough::QaGameReady));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn game_canvas_capture_survives_call_but_load_revokes_it_before_any_successor_input() {
        let source=r#"
function paint(state,props,frame) {return []}
screen hud() {return component("canvas","canvas",{"draw":"paint","capture_pointer":true,"width":100,"height":100,"accessible_label":"Gesture area"},[])}
init {ui.open("hud",[],false,0)}
scene label P
"parent"
jump done
scene label C
"child"
return
label done
"#;
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        game.step_until_interaction().unwrap();
        let token=game.scene_resource_token(rvn_core::scene_resources::ResourceTaskKind::Canvas,"hud/canvas").unwrap();
        let views=game.interface_views().unwrap();
        let order=views[0].order;let epoch=game.interface_epoch();
        let mut world=World::new();world.insert_resource(VnEngine(game));world.init_resource::<Screens>();world.resource_mut::<Screens>().views=views;
        let mut pointer=PointerState::default();pointer.epoch=epoch;
        pointer.captures.insert("mouse".into(),Capture{screen:"hud".into(),element:"canvas".into(),order,button:"left".into(),position:Vec2::new(4.0,7.0),resource_token:Some(token.clone())});
        world.resource_mut::<VnEngine>().0.execute_timer_action("call C").unwrap();
        world.resource_mut::<VnEngine>().0.step_until_interaction().unwrap();
        let views=world.resource::<VnEngine>().0.interface_views().unwrap();world.resource_mut::<Screens>().views=views;
        reconcile_capture_authority(&world,&mut pointer);
        assert_eq!(pointer.captures["mouse"].resource_token,Some(token));
        assert_eq!(pointer.captures["mouse"].position,Vec2::new(4.0,7.0));
        assert!(!pointer.suppressed.contains("mouse"));
        let save=rvn_core::save::SaveData::try_from_state(&world.resource::<VnEngine>().0.state,1,"child".into(),"main.rvn".into()).unwrap();
        world.resource_mut::<VnEngine>().0.load_data(save).unwrap();
        let views=world.resource::<VnEngine>().0.interface_views().unwrap();world.resource_mut::<Screens>().views=views;
        reconcile_capture_authority(&world,&mut pointer);
        assert!(!pointer.captures.contains_key("mouse"));assert!(pointer.suppressed.contains("mouse"));
    }
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

#[cfg(test)]
mod scoped_canvas_input_contracts {
    use super::*;
    fn world()->World {
        let source=r#"
function paint(state,props,frame){return []}
handler hit(event){set calls=calls+1}
handler renew(event){ui.close("form") ui.open("form",[],false,0)}
screen form(){return component("canvas","canvas",{"draw":"paint","events":{"focus":"renew","pointer_down":"hit"},"width":100,"height":100,"accessible_label":"Scoped gesture"},[])}
init {set calls=0}
label start
ui.open("form",[],false,0)
"ready"
"#;
        let mut engine=rvn_core::Engine::new(rvn_parser::parse(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();engine.step_until_interaction().unwrap();
        let views=engine.interface_views().unwrap();let mut world=World::new();world.insert_resource(VnEngine(engine));world.init_resource::<Screens>();world.resource_mut::<Screens>().views=views;world.init_resource::<ScriptErrorMessage>();world.init_resource::<NextState<VnState>>();world.init_resource::<Events<VnCommand>>();world
    }
    #[test]
    fn scoped_canvas_focus_replacement_rejects_original_pointer_down_before_payload(){
        let mut world=world();let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        assert!(!dispatch(&mut world,source.clone(),UiInput{screen:"form".into(),element:"canvas".into(),kind:ScreenEventKind::Focus,value:None,key:None}));
        assert_ne!(world.resource::<VnEngine>().0.state.ui.screens[0].order,source.order);
        assert!(!world.resource::<VnEngine>().0.ui_input_source_current(&source));
        let before=serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap();let history=world.resource::<VnEngine>().0.history.len();
        assert!(!dispatch(&mut world,source,UiInput{screen:"form".into(),element:"canvas".into(),kind:ScreenEventKind::PointerDown,value:Some(Value::Float(f32::NAN)),key:Some("x".repeat(1024))}));
        assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap(),before);assert_eq!(world.resource::<VnEngine>().0.history.len(),history);assert_eq!(world.resource::<VnEngine>().0.state.vars["calls"],Value::Int(0));assert!(world.resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn scoped_canvas_pointer_cancel_cannot_bypass_reopened_instance_identity(){
        let mut world=world();let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        assert!(!dispatch(&mut world,source.clone(),UiInput{screen:"form".into(),element:"canvas".into(),kind:ScreenEventKind::Focus,value:None,key:None}));
        assert_ne!(world.resource::<VnEngine>().0.state.ui.screens[0].order,source.order);
        let before=serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap();
        assert!(!dispatch(&mut world,source,UiInput{screen:"form".into(),element:"canvas".into(),kind:ScreenEventKind::PointerCancel,value:Some(Value::Float(f32::NAN)),key:None}));assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap(),before);
    }

    #[test]
    fn scoped_canvas_raw_press_cannot_acquire_a_new_instance_published_after_frame_capture(){
        let mut world=world();let old=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();crate::programmable_ui::capture_frame_for_test(&mut world,[("form".into(),old.clone())].into_iter().collect());
        world.resource_mut::<VnEngine>().0.interface_event(UiInput{screen:"form".into(),element:"canvas".into(),kind:ScreenEventKind::Focus,value:None,key:None}).unwrap();
        let views=world.resource::<VnEngine>().0.interface_views().unwrap();world.resource_mut::<Screens>().views=views;let new=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();assert_ne!(new,old);assert!(world.resource::<VnEngine>().0.ui_input_source_current(&new));
        let mut node=Node::default();let bevy::reflect::ReflectMut::Struct(fields)=node.reflect_mut()else{unreachable!()};fields.field_mut("calculated_size").unwrap().apply(&Vec2::splat(100.0));
        let entity=world.spawn((node,Style::default(),GlobalTransform::default(),CanvasSurface{screen:"form".into(),element:"canvas".into(),drawing:CanvasDrawing::default(),extent:Vec2::splat(100.0),font:Handle::default(),opacity:1.0},crate::programmable_ui::ControlSource(new))).id();world.init_resource::<bevy::ui::UiStack>();world.resource_mut::<bevy::ui::UiStack>().uinodes=vec![entity];
        let before=serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap();let mut pointer=PointerState::default();assert!(route_pointer(&mut world,&mut pointer,"mouse",ScreenEventKind::PointerDown,Vec2::ZERO,"left",Vec2::ZERO));
        assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap(),before);assert!(pointer.captures.is_empty());assert_eq!(world.resource::<VnEngine>().0.state.vars["calls"],Value::Int(0));assert!(world.resource::<ScriptErrorMessage>().0.is_empty());
    }
}

#[cfg(test)]
mod scoped_canvas_physical_producer_contracts {
    #[test]
    fn preview_canvas_budget_is_checked_before_drawing_and_clip_snapshots_are_copied(){
        let mut world=world(true);let entity=surface(&mut world);
        crate::programmable_ui::publish_canvas_input_tokens_for_test(&mut world);
        let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        crate::programmable_ui::capture_frame_for_test(&mut world,[("form".into(),source)].into_iter().collect());
        let before=serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();
        let history=format!("{:?}",world.resource::<VnEngine>().0.history.entries());
        assert!(capture_preview_canvas(&world).unwrap().0.0.pick(Vec2::ZERO).is_some());
        let drawing=CanvasDrawing{primitives:(0..4).map(|_|rvn_ui::custom_canvas::CanvasPrimitive::Text{text:"x".repeat(65536),position:[0.0,0.0],color:[1.0;4],size:16.0}).collect()};drawing.validate().unwrap();
        let source=world.get::<crate::programmable_ui::ControlSource>(entity).unwrap().clone();
        let mut entities=vec![entity];
        world.get_mut::<CanvasSurface>(entity).unwrap().drawing=drawing.clone();
        for _ in 0..4 {let mut node=Node::default();let bevy::reflect::ReflectMut::Struct(fields)=node.reflect_mut()else{unreachable!()};fields.field_mut("calculated_size").unwrap().apply(&Vec2::splat(100.0));entities.push(world.spawn((node,Style::default(),GlobalTransform::default(),CanvasSurface{screen:"form".into(),element:"canvas".into(),drawing:drawing.clone(),extent:Vec2::splat(100.0),font:Handle::default(),opacity:1.0},source.clone())).id());}
        world.resource_mut::<bevy::ui::UiStack>().uinodes=entities;
        assert!(preview_canvas_cost(&world).is_none());assert!(capture_preview_canvas(&world).is_none());
        assert_eq!(world.get::<CanvasSurface>(entity).unwrap().drawing,drawing);
        assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),before);assert_eq!(format!("{:?}",world.resource::<VnEngine>().0.history.entries()),history);
    }

    #[test]
    fn completed_image_canvas_keeps_empty_hit_when_same_interface_adds_a_canvas() {
        let mut world=world(false);
        let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        crate::programmable_ui::capture_frame_for_test(&mut world,[("form".into(),source.clone())].into_iter().collect());
        let (frame,_)=capture_preview_canvas(&world).unwrap();assert!(frame.0.pick(Vec2::ZERO).is_none());
        event(&mut world,"form","trigger");surface(&mut world);
        assert_eq!(world.resource::<VnEngine>().0.capture_ui_input_source("form"),Some(source));
        let before=serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();
        let history=format!("{:?}",world.resource::<VnEngine>().0.history.entries());
        let window=world.spawn_empty().id();
        frame.mouse(&mut world,MouseButtonInput{button:MouseButton::Left,state:ButtonState::Pressed,window},Vec2::ZERO);
        world.run_system_once(capture_pointer_producers);world.run_system_once(input);
        assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),before);
        assert_eq!(format!("{:?}",world.resource::<VnEngine>().0.history.entries()),history);
        assert_eq!(world.resource::<VnEngine>().0.state.vars["calls"],Value::Int(0));
    }
    #[test]
    fn completed_image_canvas_mouse_uses_its_drawing_and_transform_after_same_instance_update() {
        let mut world=world(true);let entity=surface(&mut world);
        let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        crate::programmable_ui::capture_frame_for_test(&mut world,[("form".into(),source)].into_iter().collect());
        let (frame,_)=capture_preview_canvas(&world).unwrap();
        event(&mut world,"controls","alter");
        *world.get_mut::<GlobalTransform>(entity).unwrap()=GlobalTransform::from_translation(Vec3::new(100.0,0.0,0.0));
        let drawing=world.resource::<Screens>().views.iter().find(|view|view.name=="form").unwrap().root.find("canvas").unwrap().drawing.clone().unwrap();
        {let mut surface=world.get_mut::<CanvasSurface>(entity).unwrap();surface.extent=Vec2::splat(200.0);surface.drawing=drawing;}
        let window=world.spawn_empty().id();
        frame.mouse(&mut world,MouseButtonInput{button:MouseButton::Left,state:ButtonState::Pressed,window},Vec2::new(25.0,0.0));
        world.run_system_once(capture_pointer_producers);world.run_system_once(input);
        let game=&world.resource::<VnEngine>().0;
        assert_eq!(game.state.vars["calls"],Value::Int(1));assert_eq!(game.state.vars["last_x"],Value::Float(75.0));
        assert_eq!(game.state.vars["last_y"],Value::Float(50.0));assert_eq!(game.state.vars["last_hit"],Value::Str("old".into()));
        assert_eq!(game.state.vars["region"],Value::Str("new".into()));assert!(world.resource::<ScriptErrorMessage>().0.is_empty());
    }

    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn world(visible: bool) -> World {
        let source = r#"
function paint(state,props,frame){return [canvas_hit(region,[0,0,100,100])]}
handler reveal(event){set visible=true}
handler alter(event){set region="new"}
handler hit(event){set calls=calls+1 set last_x=event["value"]["x"] set last_y=event["value"]["y"] set last_hit=event["value"]["hit"]}
handler moved(event){set moves=moves+1 set last_x=event["value"]["x"] set last_y=event["value"]["y"]}
screen form(){
    if visible {return component("canvas","canvas",{"draw":"paint","state":{},"capture_pointer":true,"events":{"pointer_down":"hit","pointer_move":"moved"},"width":100,"height":100,"accessible_label":"Physical gesture"},[])}
    return component("trigger","button",{"events":{"click":"reveal"}},[])
}
screen controls(){return component("alter","button",{"events":{"click":"alter"}},[])}
init{set visible=INITIAL set calls=0 set moves=0 set last_x=0 set last_y=0 set last_hit="" set region="old"}
label start
ui.open("form",[],false,0)
ui.open("controls",[],false,0)
"ready"
"#;
        let source=source.replace("INITIAL",if visible {"true"} else {"false"});
        let mut engine=rvn_core::Engine::new(rvn_parser::parse(&source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        engine.step_until_interaction().unwrap();
        let views=engine.interface_views().unwrap();
        let mut world=World::new();
        world.insert_resource(VnEngine(engine));
        world.init_resource::<Screens>();
        world.resource_mut::<Screens>().views=views;
        world.resource_mut::<Screens>().active=true;
        world.init_resource::<ScriptErrorMessage>();
        world.init_resource::<NextState<VnState>>();
        world.insert_resource(State::new(VnState::Waiting));
        world.init_resource::<crate::accessibility::Accessibility>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<PointerState>();
        world.init_resource::<Events<VnCommand>>();
        world.init_resource::<Events<MouseButtonInput>>();
        world.init_resource::<Events<CursorMoved>>();
        world.init_resource::<Events<TouchInput>>();
        world.init_resource::<Events<MouseWheel>>();
        world.init_resource::<Events<KeyboardInput>>();
        world.init_resource::<Events<WindowFocused>>();
        world.init_resource::<bevy::ui::UiStack>();
        world
    }
    fn surface(world:&mut World)->Entity {
        let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        let drawing=world.resource::<Screens>().views.iter().find(|view|view.name=="form").unwrap().root.find("canvas").unwrap().drawing.clone().unwrap();
        let mut node=Node::default();
        let bevy::reflect::ReflectMut::Struct(fields)=node.reflect_mut() else {unreachable!()};
        fields.field_mut("calculated_size").unwrap().apply(&Vec2::splat(100.0));
        let entity=world.spawn((node,Style::default(),GlobalTransform::default(),CanvasSurface{screen:"form".into(),element:"canvas".into(),drawing,extent:Vec2::splat(100.0),font:Handle::default(),opacity:1.0},crate::programmable_ui::ControlSource(source))).id();
        world.resource_mut::<bevy::ui::UiStack>().uinodes=vec![entity];
        entity
    }
    fn raw_touch(world:&mut World,phase:TouchPhase,position:Vec2)->usize {
        let window=world.spawn_empty().id();
        world.resource_mut::<Events<TouchInput>>().send(TouchInput{phase,position,window,force:None,id:7}).id
    }
    fn publish_views(world:&mut World) {
        let views=world.resource::<VnEngine>().0.interface_views().unwrap();
        crate::programmable_ui::update_views(&mut world.resource_mut::<Screens>(),views);
    }
    fn event(world:&mut World,screen:&str,element:&str) {
        world.resource_mut::<VnEngine>().0.interface_event(UiInput{screen:screen.into(),element:element.into(),kind:ScreenEventKind::Click,value:None,key:None}).unwrap();
        publish_views(world);
    }
    #[test]
    fn scoped_canvas_raw_event_keeps_empty_physical_hit_when_same_interface_adds_canvas() {
        let mut world=world(false);
        let old=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        crate::programmable_ui::capture_frame_for_test(&mut world,[("form".into(),old.clone())].into_iter().collect());
        let id=raw_touch(&mut world,TouchPhase::Started,Vec2::ZERO);
        world.run_system_once(capture_pointer_producers);
        let produced=world.resource::<CanvasPointerProducers>().touch[&id].clone();
        assert!(produced.frame.pick(Vec2::ZERO).is_none());
        event(&mut world,"form","trigger");
        assert_eq!(world.resource::<VnEngine>().0.capture_ui_input_source("form"),Some(old));
        surface(&mut world);
        assert!(capture_physical_frame(&world).pick(Vec2::ZERO).is_some());
        world.run_system_once(capture_pointer_producers);
        assert!(std::sync::Arc::ptr_eq(&produced.frame,&world.resource::<CanvasPointerProducers>().touch[&id].frame));
        let before=serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap();
        let history=world.resource::<VnEngine>().0.history.len();
        world.run_system_once(input);
        assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap(),before);
        assert_eq!(world.resource::<VnEngine>().0.history.len(),history);
        assert_eq!(world.resource::<VnEngine>().0.state.vars["calls"],Value::Int(0));
        assert!(world.resource::<PointerState>().captures.is_empty());
        assert!(world.resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn scoped_canvas_raw_event_uses_produced_geometry_and_hit_regions_after_same_instance_update() {
        let mut world=world(true);
        let entity=surface(&mut world);
        let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        crate::programmable_ui::capture_frame_for_test(&mut world,[("form".into(),source.clone())].into_iter().collect());
        raw_touch(&mut world,TouchPhase::Started,Vec2::new(25.0,0.0));
        world.run_system_once(capture_pointer_producers);
        event(&mut world,"controls","alter");
        assert_eq!(world.resource::<VnEngine>().0.capture_ui_input_source("form"),Some(source));
        *world.get_mut::<GlobalTransform>(entity).unwrap()=GlobalTransform::from_translation(Vec3::new(100.0,0.0,0.0));
        let new_drawing=world.resource::<Screens>().views.iter().find(|view|view.name=="form").unwrap().root.find("canvas").unwrap().drawing.clone().unwrap();
        { let mut canvas=world.get_mut::<CanvasSurface>(entity).unwrap();canvas.extent=Vec2::splat(200.0);canvas.drawing=new_drawing; }
        assert!(pick(&world,Vec2::new(25.0,0.0)).is_none());
        world.run_system_once(input);
        let game=&world.resource::<VnEngine>().0;
        assert_eq!(game.state.vars["calls"],Value::Int(1));
        assert_eq!(game.state.vars["last_x"],Value::Float(75.0));
        assert_eq!(game.state.vars["last_y"],Value::Float(50.0));
        assert_eq!(game.state.vars["last_hit"],Value::Str("old".into()));
        assert_eq!(game.state.vars["region"],Value::Str("new".into()));
        assert!(world.resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn scoped_canvas_captured_raw_move_keeps_outside_coordinates_from_its_producer_frame() {
        let mut world=world(true);
        let entity=surface(&mut world);
        let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        crate::programmable_ui::capture_frame_for_test(&mut world,[("form".into(),source)].into_iter().collect());
        raw_touch(&mut world,TouchPhase::Started,Vec2::ZERO);
        raw_touch(&mut world,TouchPhase::Moved,Vec2::new(150.0,0.0));
        world.run_system_once(capture_pointer_producers);
        *world.get_mut::<GlobalTransform>(entity).unwrap()=GlobalTransform::from_translation(Vec3::new(100.0,0.0,0.0));
        world.run_system_once(input);
        let game=&world.resource::<VnEngine>().0;
        assert_eq!(game.state.vars["calls"],Value::Int(1));
        assert_eq!(game.state.vars["moves"],Value::Int(1));
        assert_eq!(game.state.vars["last_x"],Value::Float(200.0));
        assert_eq!(game.state.vars["last_y"],Value::Float(50.0));
        assert!(world.resource::<PointerState>().captures.contains_key("touch:7"));
        assert!(world.resource::<ScriptErrorMessage>().0.is_empty());
    }
}

#[cfg(test)]
mod scoped_canvas_keyboard_producer_contracts {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use rvn_core::scene_resources::{ResourceTaskKind,ResourceToken};

    fn world(initial:i64)->World {
        let source=r#"
function paint(state,props,frame){return [canvas_hit("area",[0,0,100,100])]}
handler keyed(event){set calls=calls+1}
handler input_mode(event){set mode=1}
handler canvas_mode(event){set mode=0}
screen form(){
 if mode==1 {return component("target","input",{"binding":"text"},[])}
 return component("target","canvas",{"draw":"paint","state":{},"events":{"key":"keyed"},"width":100,"height":100},[])
}
screen controls(){return component("root","panel",{},[component("input","button",{"events":{"click":"input_mode"}},[]),component("canvas","button",{"events":{"click":"canvas_mode"}},[])])}
init{set mode=INITIAL set calls=0 set text="original"}
scene label P
ui.open("form",[],false,0)
ui.open("controls",[],false,0)
"ready"
"#.replace("INITIAL",&initial.to_string());
        let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(&source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        engine.step_until_interaction().unwrap();engine.renderer.scene_batch=None;
        let views=engine.interface_views().unwrap();let mut world=World::new();world.insert_resource(VnEngine(engine));world.init_resource::<Screens>();
        {let mut screens=world.resource_mut::<Screens>();screens.views=views;screens.active=true;screens.keyboard_focus=Some(("form".into(),"target".into()));}
        world.init_resource::<ScriptErrorMessage>();world.init_resource::<NextState<VnState>>();world.insert_resource(State::new(VnState::Waiting));
        world.init_resource::<crate::accessibility::Accessibility>();world.init_resource::<ButtonInput<KeyCode>>();world.init_resource::<PointerState>();
        world.init_resource::<Events<VnCommand>>();world.init_resource::<Events<MouseButtonInput>>();world.init_resource::<Events<CursorMoved>>();world.init_resource::<Events<TouchInput>>();world.init_resource::<Events<MouseWheel>>();world.init_resource::<Events<KeyboardInput>>();world.init_resource::<Events<WindowFocused>>();world.init_resource::<bevy::ui::UiStack>();
        world.init_resource::<crate::scene_publication::ScenePresentation>();
        capture(&mut world);crate::programmable_ui::publish_canvas_input_tokens_for_test(&mut world);world
    }
    fn sources(world:&World)->HashMap<String,UiInputSource> {
        world.resource::<Screens>().views.iter().filter_map(|view|world.resource::<VnEngine>().0.capture_ui_input_source(&view.name).map(|source|(view.name.clone(),source))).collect()
    }
    fn capture(world:&mut World) {let sources=sources(world);crate::programmable_ui::capture_frame_for_test(world,sources);}
    fn token(world:&World)->ResourceToken {world.resource::<VnEngine>().0.scene_resource_token(ResourceTaskKind::Canvas,"form/target").unwrap()}
    fn mutate(world:&mut World,element:&str) {
        world.resource_mut::<VnEngine>().0.interface_event(UiInput{screen:"controls".into(),element:element.into(),kind:ScreenEventKind::Click,value:None,key:None}).unwrap();
        let views=world.resource::<VnEngine>().0.interface_views().unwrap();world.resource_mut::<Screens>().views=views;world.resource_mut::<VnEngine>().0.renderer.scene_batch=None;
    }
    fn key(world:&mut World,value:&str)->usize {
        let window=world.spawn_empty().id();world.resource_mut::<Events<KeyboardInput>>().send(KeyboardInput{key_code:KeyCode::KeyK,logical_key:Key::Character(value.into()),state:ButtonState::Pressed,window}).id
    }
    fn state(world:&World)->serde_json::Value {serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap()}
    fn history(world:&World)->serde_json::Value {serde_json::to_value(world.resource::<VnEngine>().0.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>()).unwrap()}
    fn positive_new_key(world:&mut World) {
        let before=world.resource::<VnEngine>().0.history.len();key(world,"k");capture(world);world.run_system_once(input);
        assert_eq!(world.resource::<VnEngine>().0.state.vars["calls"],Value::Int(1));assert_eq!(world.resource::<VnEngine>().0.history.len(),before+1);assert!(world.resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn scoped_canvas_keyboard_same_interface_renewal_cannot_adopt_a_new_canvas_token(){
        for produced_before_model_change in [false,true] {
            let mut world=world(0);let old_interface=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();let old_token=token(&world);
            let id=key(&mut world,&"oversized payload ".repeat(40));
            if produced_before_model_change {capture(&mut world);}
            mutate(&mut world,"input");assert!(world.resource::<VnEngine>().0.scene_resource_token(ResourceTaskKind::Canvas,"form/target").is_none());mutate(&mut world,"canvas");
            let new_token=token(&world);assert_ne!(old_token,new_token);assert_eq!(world.resource::<VnEngine>().0.capture_ui_input_source("form"),Some(old_interface));
            // Even if Core changed before capture, the producer sees the old
            // published appearance/token, never a fresh same-alias lookup.
            capture(&mut world);
            assert!(!crate::programmable_ui::queued_native_key_current(world.resource::<Screens>(),world.resource::<VnEngine>(),id));
            crate::programmable_ui::publish_canvas_input_tokens_for_test(&mut world);capture(&mut world);
            assert!(!crate::programmable_ui::queued_native_key_current(world.resource::<Screens>(),world.resource::<VnEngine>(),id));
            let before=state(&world);let old_history=history(&world);world.run_system_once(input);
            assert_eq!(state(&world),before);assert_eq!(history(&world),old_history);assert_eq!(world.resource::<VnEngine>().0.state.vars["calls"],Value::Int(0));assert!(world.resource::<ScriptErrorMessage>().0.is_empty());
            positive_new_key(&mut world);
        }
    }
    #[test]
    fn scoped_canvas_keyboard_input_to_canvas_conversion_rejects_before_key_payload(){
        let mut world=world(1);let old_interface=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        let id=key(&mut world,&"invalid old input ".repeat(40));capture(&mut world);mutate(&mut world,"canvas");
        assert_eq!(world.resource::<VnEngine>().0.capture_ui_input_source("form"),Some(old_interface));assert!(world.resource::<VnEngine>().0.scene_resource_token(ResourceTaskKind::Canvas,"form/target").is_some());
        crate::programmable_ui::publish_canvas_input_tokens_for_test(&mut world);capture(&mut world);
        assert!(!crate::programmable_ui::queued_native_key_current(world.resource::<Screens>(),world.resource::<VnEngine>(),id));
        let before=state(&world);let old_history=history(&world);world.run_system_once(input);
        assert_eq!(state(&world),before);assert_eq!(history(&world),old_history);assert_eq!(world.resource::<VnEngine>().0.state.vars["text"],Value::Str("original".into()));assert!(world.resource::<ScriptErrorMessage>().0.is_empty());
        positive_new_key(&mut world);
    }
    #[test]
    fn scoped_canvas_pointer_does_not_capture_an_unpublished_new_token_from_the_current_model(){
        let mut world=world(0);let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();let old_token=token(&world);
        let drawing=world.resource::<Screens>().views.iter().find(|view|view.name=="form").unwrap().root.drawing.clone().unwrap();
        let mut node=Node::default();let bevy::reflect::ReflectMut::Struct(fields)=node.reflect_mut()else{unreachable!()};fields.field_mut("calculated_size").unwrap().apply(&Vec2::splat(100.0));
        let entity=world.spawn((node,Style::default(),GlobalTransform::default(),CanvasSurface{screen:"form".into(),element:"target".into(),drawing,extent:Vec2::splat(100.0),font:Handle::default(),opacity:1.0},crate::programmable_ui::ControlSource(source))).id();world.resource_mut::<bevy::ui::UiStack>().uinodes=vec![entity];
        mutate(&mut world,"input");mutate(&mut world,"canvas");assert_ne!(token(&world),old_token);
        let window=world.spawn_empty().id();let id=world.resource_mut::<Events<TouchInput>>().send(TouchInput{phase:TouchPhase::Started,position:Vec2::ZERO,window,force:None,id:9}).id;
        world.run_system_once(capture_pointer_producers);
        assert!(world.resource::<CanvasPointerProducers>().touch[&id].frame.pick(Vec2::ZERO).is_none());
        crate::programmable_ui::publish_canvas_input_tokens_for_test(&mut world);world.run_system_once(capture_pointer_producers);
        assert!(world.resource::<CanvasPointerProducers>().touch[&id].frame.pick(Vec2::ZERO).is_none());
        let before=state(&world);world.run_system_once(input);assert_eq!(state(&world),before);assert!(world.resource::<PointerState>().captures.is_empty());assert!(world.resource::<ScriptErrorMessage>().0.is_empty());
    }
}
