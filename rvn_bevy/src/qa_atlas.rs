//! Opt-in verification of the owned Atlas demo in a real Linux Bevy window.
//! A temporary project copy and a fresh RVN_QA_OUTPUT directory are required.
//! UI events run the authored handlers; pixels come from ScreenshotManager.
//! This does not certify OS pointer hardware, speech services or Web support.
use crate::{
    accessibility::Accessibility,
    components::{DialogueText, VnSprite},
    custom_canvas::CanvasSurface,
    layered_characters::{CompositionRoot, LayerTarget},
    project_paths::ProjectPaths,
    resources::{ScriptErrorMessage, TypewriterState, VnEngine, VnState},
    vn_command::PlayerInput,
};
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::ScreenshotManager,
    window::{CursorMoved, PrimaryWindow},
};
use rvn_core::{ui::UiInput, Interaction};
use rvn_parser::Value;
use rvn_ui::{
    custom_canvas::{flatten, hit_test, transform_point, CanvasPrimitive},
    programmable::ScreenEventKind,
};
use std::{collections::BTreeMap, path::PathBuf, time::Instant};

const TIMEOUT: f64 = 45.0;
const PHASE_SETTLE: f64 = 0.65;
const IMAGES: &[&str] = &[
    "scenes/harbor.png",
    "scenes/forest.png",
    "scenes/temple.png",
    "maps/world.png",
    "ui/frame.png",
    "portraits/player.png",
    "portraits/keeper.png",
    "icons/boussole.png",
    "icons/lanterne.png",
    "icons/grappin.png",
    "icons/sceau.png",
    "icons/cle.png",
    "icons/cristal.png",
    "layers/keeper_body.png",
    "layers/keeper_coat.png",
    "layers/keeper_neutral.png",
    "layers/keeper_smile.png",
    "layers/keeper_lantern.png",
    "sprites/lantern/default.png",
    "sprites/lantern/bright.png",
    "cgs/atlas_memory.png",
];

#[derive(Resource)]
struct Run {
    started: Instant,
    phase: usize,
    entered: f64,
    directory: PathBuf,
    images: Vec<(String, Handle<Image>)>,
    captures: Vec<String>,
    checks: Vec<String>,
    snapshots: Vec<serde_json::Value>,
    travel_pc: Option<usize>,
    advances: usize,
    choices: usize,
    finishing: Option<f64>,
    failure: Option<String>,
    pointer: Option<(Entity, Vec2)>,
}

fn number(value: &Value) -> Option<f32> {
    match value {
        Value::Int(value) => Some(*value as f32),
        Value::Float(value) => Some(*value),
        _ => None,
    }
}
fn global<'a>(world: &'a World, name: &str) -> Result<&'a Value, String> {
    world
        .resource::<VnEngine>()
        .0
        .state
        .vars
        .get(name)
        .ok_or_else(|| format!("Missing Atlas global '{name}'"))
}
fn equal(world: &World, name: &str, expected: Value) -> Result<(), String> {
    let actual = global(world, name)?;
    if actual == &expected {
        Ok(())
    } else {
        Err(format!("{name}: expected {expected:?}, found {actual:?}"))
    }
}
fn numeric_equal(world: &World, name: &str, expected: f32) -> Result<(), String> {
    if number(global(world, name)?).is_some_and(|value| (value - expected).abs() < 0.001) {
        Ok(())
    } else {
        Err(format!(
            "{name}: expected number {expected}, found {:?}",
            global(world, name)?
        ))
    }
}
fn settled(world: &World, page: i64) -> bool {
    matches!(global(world, "atlas_page"), Ok(Value::Int(value)) if *value == page)
        && global(world, "atlas_view")
            .ok()
            .and_then(number)
            .is_some_and(|value| (value - page as f32).abs() < 0.01)
}
fn window(world: &mut World) -> Result<Entity, String> {
    world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .get_single(world)
        .map_err(|problem| problem.to_string())
}
fn flush(world: &mut World) {
    for command in world.resource_mut::<VnEngine>().0.renderer.take_pending() {
        world.send_event(command);
    }
}
fn event(
    world: &mut World,
    element: &str,
    kind: ScreenEventKind,
    value: Option<Value>,
) -> Result<(), String> {
    world
        .resource_mut::<VnEngine>()
        .0
        .interface_event(UiInput {
            screen: "atlas".into(),
            element: element.into(),
            kind,
            value,
            key: None,
        })
        .map_err(|problem| problem.to_string())?;
    flush(world);
    Ok(())
}
fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let window = window(world)?;
    let path = world.resource::<Run>().directory.join(name);
    if path.exists() {
        return Err(format!("QA capture already exists: {}", path.display()));
    }
    world
        .resource_mut::<ScreenshotManager>()
        .save_screenshot_to_disk(window, path)
        .map_err(|problem| problem.to_string())?;
    let engine = &world.resource::<VnEngine>().0;
    let snapshot = serde_json::json!({
        "capture":name,
        "phase":world.resource::<Run>().phase,
        "runtime_state":format!("{:?}", world.resource::<State<VnState>>().get()),
        "game":engine.state,
    });
    let mut run = world.resource_mut::<Run>();
    run.captures.push(name.into());
    run.snapshots.push(snapshot);
    Ok(())
}
fn check(world: &mut World, message: &str) {
    world.resource_mut::<Run>().checks.push(message.into());
}
fn advance(world: &mut World) {
    world.send_event(PlayerInput::Advance);
    world.resource_mut::<Run>().advances += 1;
}
fn next(world: &mut World, now: f64) {
    let mut run = world.resource_mut::<Run>();
    run.phase += 1;
    run.entered = now;
}

fn initial_guard(world: &World, directory: PathBuf) -> Result<Run, String> {
    let root = world
        .resource::<ProjectPaths>()
        .root
        .canonicalize()
        .map_err(|problem| problem.to_string())?;
    let temporary = std::env::temp_dir()
        .canonicalize()
        .map_err(|problem| problem.to_string())?;
    if !root.starts_with(&temporary) {
        return Err(
            "RVN_QA_ATLAS requires an isolated project copy under the system temporary directory"
                .into(),
        );
    }
    if directory.join("native.json").exists() {
        return Err(
            "RVN_QA_OUTPUT already contains native.json; use a fresh evidence directory".into(),
        );
    }
    let source =
        std::fs::read_to_string(root.join("main.rvn")).map_err(|problem| problem.to_string())?;
    if !source.contains("function atlas_keeper_portrait")
        || !source.contains("label atlas_visit_havre")
    {
        return Err(
            "RVN_QA_ATLAS only supports the original Atlas demo, not an arbitrary game".into(),
        );
    }
    if world
        .resource::<ProjectPaths>()
        .saves
        .join("quicksave.json")
        .exists()
    {
        return Err(
            "The temporary Atlas copy already has a quicksave; QA needs clean isolated state"
                .into(),
        );
    }
    Ok(Run {
        started: Instant::now(),
        phase: 0,
        entered: 0.0,
        directory,
        images: Vec::new(),
        captures: Vec::new(),
        checks: Vec::new(),
        snapshots: Vec::new(),
        travel_pc: None,
        advances: 0,
        choices: 0,
        finishing: None,
        failure: None,
        pointer: None,
    })
}
fn loaded(world: &World) -> Result<bool, String> {
    let assets = world.resource::<AssetServer>();
    let images = world.resource::<Assets<Image>>();
    for (name, handle) in &world.resource::<Run>().images {
        if let Some(bevy::asset::LoadState::Failed(problem)) = assets.get_load_state(handle.id()) {
            return Err(format!("Atlas image '{name}' failed to load: {problem}"));
        }
        if images.get(handle).is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}
fn canvas_ready(world: &mut World) -> bool {
    let font_handles: Vec<_> = world
        .query::<(&CanvasSurface, &Node)>()
        .iter(world)
        .filter(|(surface, node)| {
            surface.screen == "atlas"
                && surface.element == "atlas_surface"
                && node.size().min_element() > 100.0
        })
        .map(|(surface, _)| surface.font.clone())
        .collect();
    let fonts = world.resource::<Assets<Font>>();
    font_handles
        .iter()
        .any(|handle| fonts.get(handle).is_some())
}
fn hover_marker(world: &mut World, id: &str) -> Result<(), String> {
    // Use the authored hit shapes/transforms, not an approximation of a screenshot.
    let views = world
        .resource::<VnEngine>()
        .0
        .interface_views()
        .map_err(|problem| problem.to_string())?;
    let canvas = views
        .iter()
        .find(|view| view.name == "atlas")
        .and_then(|view| view.root.find("atlas_surface"))
        .ok_or("Atlas canvas is absent")?;
    let items = flatten(canvas.drawing.as_ref().ok_or("Atlas drawing is absent")?)?;
    let point = items
        .iter()
        .find_map(|item| match &item.primitive {
            CanvasPrimitive::Hit { id: actual, rect } if actual == id => Some(transform_point(
                item.matrix,
                [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5],
            )),
            _ => None,
        })
        .ok_or_else(|| format!("Missing authored marker '{id}'"))?;
    if hit_test(&items, point).as_deref() != Some(id) {
        return Err(format!("Authored marker '{id}' is occluded at its centre"));
    }
    let entity = world
        .query::<(Entity, &CanvasSurface)>()
        .iter(world)
        .find(|(_, surface)| surface.screen == "atlas" && surface.element == "atlas_surface")
        .map(|(entity, _)| entity)
        .ok_or("Atlas canvas has no native UI entity")?;
    let surface = world
        .get::<CanvasSurface>(entity)
        .ok_or("Atlas surface vanished")?;
    let size = world
        .get::<Node>(entity)
        .ok_or("Atlas native node is absent")?
        .size();
    let position = world
        .get::<GlobalTransform>(entity)
        .ok_or("Atlas native transform is absent")?
        .transform_point(
            ((Vec2::from_array(point) / surface.extent - Vec2::splat(0.5)) * size).extend(0.0),
        )
        .truncate();
    let window = window(world)?;
    world
        .get_mut::<Window>(window)
        .ok_or("QA window is absent")?
        .set_cursor_position(Some(position));
    world.send_event(CursorMoved {
        window,
        position,
        delta: None,
    });
    world.resource_mut::<Run>().pointer = Some((window, position));
    Ok(())
}

fn clear_events<E: Event>(world: &mut World) {
    if let Some(mut events) = world.get_resource_mut::<Events<E>>() {
        events.clear();
    }
}

/// Registered only for RVN_QA_ATLAS + RVN_QA_OUTPUT, never for normal gameplay.
/// The owned visible test window is not an invitation to accept live hardware
/// input. Preserve only the driver's authored pointer position and direct
/// PlayerInput dialogue/choice events, which run the real engine normally.
pub(crate) fn isolate_inputs(world: &mut World) {
    if let Some(mut keys) = world.get_resource_mut::<ButtonInput<KeyCode>>() {
        keys.reset_all();
    }
    if let Some(mut mouse) = world.get_resource_mut::<ButtonInput<MouseButton>>() {
        mouse.reset_all();
    }
    if let Some(mut pads) = world.get_resource_mut::<ButtonInput<GamepadButton>>() {
        pads.reset_all();
    }
    clear_events::<bevy::input::keyboard::KeyboardInput>(world);
    clear_events::<bevy::input::mouse::MouseButtonInput>(world);
    clear_events::<bevy::input::mouse::MouseWheel>(world);
    clear_events::<bevy::input::touch::TouchInput>(world);
    clear_events::<bevy::window::Ime>(world);
    clear_events::<bevy::window::WindowFocused>(world);
    clear_events::<CursorMoved>(world);
    let pointer = world.get_resource::<Run>().and_then(|run| run.pointer);
    // Update only this app's primary window resource, not the OS cursor/focus.
    if let Ok(window) = window(world) {
        if let Some(mut own_window) = world.get_mut::<Window>(window) {
            own_window.set_cursor_position(pointer.map(|(_, position)| position));
        }
    }
    if let Some((window, position)) = pointer {
        world.send_event(CursorMoved {
            window,
            position,
            delta: None,
        });
    }
}
fn hover(world: &World) -> Option<i64> {
    let screen = world
        .resource::<VnEngine>()
        .0
        .state
        .ui
        .screens
        .iter()
        .find(|screen| screen.name == "atlas")?;
    let Value::Dict(state) = &screen.canvas_states.get("atlas_surface")?.state else {
        return None;
    };
    match state.get("hover")? {
        Value::Int(value) => Some(*value),
        _ => None,
    }
}
fn dialogue(world: &mut World) -> Result<Option<String>, String> {
    if *world.resource::<State<VnState>>().get() != VnState::Waiting {
        return Ok(None);
    }
    let Some(Interaction::Dialogue { text, .. }) = world
        .resource::<VnEngine>()
        .0
        .current_interaction()
        .map_err(|problem| problem.to_string())?
    else {
        return Ok(None);
    };
    if world.resource::<TypewriterState>().typing {
        world.send_event(PlayerInput::SkipTypewriter);
        return Ok(None);
    }
    let visible: String = world
        .query_filtered::<&Text, With<DialogueText>>()
        .iter(world)
        .flat_map(|text| text.sections.iter().map(|section| section.value.as_str()))
        .collect();
    if visible != text {
        return Ok(None);
    }
    Ok(Some(text))
}
fn keeper_layers(world: &mut World, expected: &[&str]) -> Result<(), String> {
    let roots: Vec<_> = world
        .query::<(Entity, &VnSprite, &CompositionRoot)>()
        .iter(world)
        .filter(|(_, sprite, _)| sprite.id == "keeper")
        .map(|(entity, _, _)| entity)
        .collect();
    if roots.len() != 1 {
        return Err(format!(
            "Expected one native keeper composition root, found {}",
            roots.len()
        ));
    }
    let mut actual = BTreeMap::new();
    for (entity, target, sprite, transform, texture) in world
        .query::<(Entity, &LayerTarget, &Sprite, &Transform, &Handle<Image>)>()
        .iter(world)
    {
        if target.character != "keeper" {
            continue;
        }
        let image = world
            .resource::<Assets<Image>>()
            .get(texture)
            .ok_or_else(|| format!("Keeper layer '{}' texture is not resident", target.layer))?;
        if world
            .get::<Parent>(entity)
            .is_none_or(|parent| parent.get() != roots[0])
        {
            return Err("Keeper layer is not attached to its composition root".into());
        }
        if !transform
            .translation
            .truncate()
            .abs_diff_eq(Vec2::ZERO, 0.001)
            || sprite
                .custom_size
                .is_none_or(|size| !size.abs_diff_eq(Vec2::new(367.2, 612.0), 0.001))
        {
            return Err(format!(
                "Keeper layer '{}' has an unexpected full-canvas alignment",
                target.layer
            ));
        }
        actual.insert(target.layer.clone(), image.size());
    }
    let mut expected: Vec<_> = expected.iter().map(|name| name.to_string()).collect();
    expected.sort();
    if actual.keys().cloned().collect::<Vec<_>>() != expected {
        return Err(format!(
            "Keeper layers differ: expected {expected:?}, found {:?}",
            actual.keys().collect::<Vec<_>>()
        ));
    }
    // The approved original image edits differ by one source pixel in height.
    // All rendered layers still use the identical registered 600×1000 canvas.
    if let Some(reference) = actual.values().next() {
        if actual
            .values()
            .any(|size| size.x.abs_diff(reference.x) > 1 || size.y.abs_diff(reference.y) > 1)
        {
            return Err(format!("Selected keeper source canvases differ by more than the explicit one-pixel tolerance: {actual:?}"));
        }
    }
    world
        .resource_mut::<Run>()
        .snapshots
        .push(serde_json::json!({
            "keeper_source_dimensions":actual,
            "maximum_source_dimension_difference_px":1,
            "registered_native_size":[367.2,612.0],
        }));
    Ok(())
}
fn png_size(bytes: &[u8]) -> Option<[u32; 2]> {
    if bytes.len() < 36
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
        || &bytes[bytes.len() - 12..bytes.len() - 4] != b"\0\0\0\0IEND"
    {
        return None;
    }
    Some([
        u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    ])
}
fn write_report(world: &World, result: &str) -> Result<(), String> {
    let run = world.resource::<Run>();
    let captures: Vec<_> = run
        .captures
        .iter()
        .map(|name| {
            let path = run.directory.join(name);
            let size = std::fs::read(&path).ok().and_then(|bytes| png_size(&bytes));
            serde_json::json!({"file":name,"png_size":size})
        })
        .collect();
    let report = serde_json::json!({
        "result":result, "platform":std::env::consts::OS,
        "method":"Real Bevy GPU window; authored Engine.interface_event handlers, native CursorMoved geometry and PlayerInput dialogue/choice events",
        "limits":["No OS hardware pointer claim", "No speech-service, Windows or Web certification"],
        "project":world.resource::<ProjectPaths>().root,
        "elapsed_seconds":run.started.elapsed().as_secs_f64(), "phase":run.phase,
        "travel_wait_pc":run.travel_pc,
        "error":run.failure, "checks":run.checks,
        "narrative_advances":run.advances, "narrative_choices":run.choices,
        "captures":captures, "snapshots":run.snapshots,
        "final_game":world.resource::<VnEngine>().0.state,
    });
    std::fs::write(
        run.directory.join("native.json"),
        serde_json::to_vec_pretty(&report).map_err(|problem| problem.to_string())?,
    )
    .map_err(|problem| problem.to_string())
}
fn fail(world: &mut World, message: String, now: f64) {
    eprintln!("Atlas native QA failed: {message}");
    {
        let mut run = world.resource_mut::<Run>();
        run.failure = Some(message);
        run.finishing = Some(now);
    }
    let _ = capture(world, "failed_native_window.png");
    let _ = write_report(world, "fail");
}

pub(crate) fn drive(world: &mut World) {
    if !world.contains_resource::<Run>() {
        let directory = PathBuf::from(
            std::env::var_os("RVN_QA_OUTPUT").expect("QaPlugin requires RVN_QA_OUTPUT"),
        );
        match initial_guard(world, directory.clone()) {
            Ok(mut run) => {
                run.images = IMAGES
                    .iter()
                    .map(|name| {
                        (
                            (*name).into(),
                            world
                                .resource::<AssetServer>()
                                .load::<Image>((*name).to_string()),
                        )
                    })
                    .collect();
                world.insert_resource(run);
            }
            Err(message) => {
                eprintln!("Atlas QA refused startup: {message}");
                let _ = std::fs::write(
                    directory.join("native-startup-error.json"),
                    serde_json::to_vec_pretty(
                        &serde_json::json!({"result":"fail","error":message}),
                    )
                    .unwrap(),
                );
                world.send_event(AppExit::Error(std::num::NonZeroU8::new(1).unwrap()));
                return;
            }
        }
    }
    let now = world.resource::<Run>().started.elapsed().as_secs_f64();
    if let Some(finished) = world.resource::<Run>().finishing {
        if now - finished > 0.6 || now >= TIMEOUT {
            let failed = world.resource::<Run>().failure.is_some();
            let _ = write_report(world, if failed { "fail" } else { "pass" });
            world.send_event(if failed {
                AppExit::Error(std::num::NonZeroU8::new(1).unwrap())
            } else {
                AppExit::Success
            });
        }
        return;
    }
    if now > TIMEOUT - 0.6 {
        let phase = world.resource::<Run>().phase;
        fail(
            world,
            format!("45-second Atlas QA timeout at phase {phase}"),
            now,
        );
        return;
    }
    if *world.resource::<State<VnState>>().get() == VnState::Error
        || !world.resource::<ScriptErrorMessage>().0.is_empty()
    {
        let message = world.resource::<ScriptErrorMessage>().0.clone();
        fail(world, format!("Runtime diagnostic: {message}"), now);
        return;
    }
    // Enable only this isolated Window resource; never focus an OS application.
    if let Ok(window) = window(world) {
        if let Some(mut window) = world.get_mut::<Window>(window) {
            window.focused = true;
        }
    }
    if *world.resource::<State<VnState>>().get() == VnState::TitleScreen {
        if now > 1.0 {
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        return;
    }
    match loaded(world) {
        Ok(true) => {}
        Ok(false) => return,
        Err(message) => {
            fail(world, message, now);
            return;
        }
    }
    if now < 1.8 || now - world.resource::<Run>().entered < PHASE_SETTLE {
        return;
    }
    let phase = world.resource::<Run>().phase;
    let result = (|| -> Result<bool, String> {
        match phase {
            0 => {
                if !settled(world, 1) || !canvas_ready(world) {
                    return Ok(false);
                }
                equal(world, "atlas_open", Value::Bool(true))?;
                equal(world, "atlas_reduced_motion", Value::Bool(false))?;
                equal(world, "atlas_high_contrast", Value::Bool(false))?;
                equal(world, "atlas_self_voicing", Value::Bool(false))?;
                numeric_equal(world, "atlas_text_scale", 1.0)?;
                if world
                    .resource::<VnEngine>()
                    .0
                    .state
                    .motions
                    .waiting
                    .as_deref()
                    != Some("ui:atlas/atlas_root")
                {
                    return Err(
                        "The initial Atlas must be waiting on its authored lifetime fence".into(),
                    );
                }
                capture(world, "01_inventory.png")?;
                check(world,"Initial native inventory renders with real fonts, original images and the saved modal lifetime fence");
                event(world, "atlas_identity_tab", ScreenEventKind::Click, None)?;
            }
            1 => {
                if !settled(world, 0) || !canvas_ready(world) {
                    return Ok(false);
                }
                capture(world, "02_identity.png")?;
                event(
                    world,
                    "atlas_reduce_motion",
                    ScreenEventKind::Change,
                    Some(Value::Bool(true)),
                )?;
                event(
                    world,
                    "atlas_high_contrast",
                    ScreenEventKind::Change,
                    Some(Value::Bool(true)),
                )?;
                event(
                    world,
                    "atlas_text_scale",
                    ScreenEventKind::Change,
                    Some(Value::Float(1.15)),
                )?;
            }
            2 => {
                equal(world, "atlas_reduced_motion", Value::Bool(true))?;
                equal(world, "atlas_high_contrast", Value::Bool(true))?;
                numeric_equal(world, "atlas_text_scale", 1.15)?;
                let access = &world.resource::<Accessibility>().settings;
                if !access.reduced_motion
                    || !access.high_contrast
                    || (access.text_scale - 1.15).abs() > 0.001
                {
                    return Err("Authored preference handlers did not reach the native accessibility policy".into());
                }
                capture(world, "03_identity_preferences.png")?;
                check(world,"Native toggles and slider update global bindings and the actual reduced-motion, high-contrast and text-scale policy");
                event(
                    world,
                    "atlas_reduce_motion",
                    ScreenEventKind::Change,
                    Some(Value::Bool(false)),
                )?;
                event(
                    world,
                    "atlas_high_contrast",
                    ScreenEventKind::Change,
                    Some(Value::Bool(false)),
                )?;
                event(
                    world,
                    "atlas_text_scale",
                    ScreenEventKind::Change,
                    Some(Value::Float(1.0)),
                )?;
                event(world, "atlas_map_tab", ScreenEventKind::Click, None)?;
            }
            3 => {
                if !settled(world, 2) || !canvas_ready(world) {
                    return Ok(false);
                }
                numeric_equal(world, "atlas_text_scale", 1.0)?;
                capture(world, "04_map.png")?;
                hover_marker(world, "marker_2")?;
            }
            4 => {
                if hover(world) != Some(2) {
                    return Ok(false);
                }
                capture(world, "05_map_hover.png")?;
                check(world,"Native CursorMoved input uses the real affine canvas hit geometry and displays the hovered destination");
                equal(world, "atlas_marker", Value::Int(0))?;
                let pc = world.resource::<VnEngine>().0.state.pc;
                world.resource_mut::<Run>().travel_pc = Some(pc);
                event(world, "atlas_travel", ScreenEventKind::Click, None)?;
                if world
                    .resource::<VnEngine>()
                    .0
                    .state
                    .ui
                    .screens
                    .iter()
                    .any(|screen| screen.name == "atlas")
                {
                    return Err("Travel handler failed to close the modal Atlas".into());
                }
                if world.resource::<VnEngine>().0.state.pc != pc {
                    return Err("Travel handler advanced narrative before the motion clock released the wait".into());
                }
            }
            5 => {
                let Some(text) = dialogue(world)? else {
                    return Ok(false);
                };
                if !text.contains("gardienne du phare") {
                    return Err(format!(
                        "Expected the first keeper encounter after travel, found '{text}'"
                    ));
                }
                if world.resource::<Run>().advances != 0 {
                    return Err("Travel required an extra narrative Advance event".into());
                }
                if world
                    .resource::<VnEngine>()
                    .0
                    .state
                    .motions
                    .waiting
                    .is_some()
                {
                    return Err(
                        "The normal animation clock did not release the closed menu fence".into(),
                    );
                }
                equal(world, "atlas_location", Value::Str("havre".into()))?;
                keeper_layers(world, &["body", "neutral"])?;
                capture(world, "06_keeper_neutral.png")?;
                check(world,"Travel runs the real handler then the normal Bevy motion clock resumes the first harbour dialogue with zero Advance events");
                advance(world);
            }
            6 => {
                let Some(text) = dialogue(world)? else {
                    return Ok(false);
                };
                if !text.contains("manteau") {
                    return Err(format!("Expected the travel-coat dialogue, found '{text}'"));
                }
                keeper_layers(world, &["body", "coat", "neutral"])?;
                capture(world, "07_keeper_coat.png")?;
                check(world,"Changing the keeper outfit preserves its neutral expression; registered native origins/sizes are identical, with source canvas dimensions recorded under an explicit one-pixel tolerance");
                advance(world);
            }
            7 => {
                let Some(text) = dialogue(world)? else {
                    return Ok(false);
                };
                if !text.contains("Un nom à inscrire") {
                    return Err(format!(
                        "Expected the named story introduction, found '{text}'"
                    ));
                }
                advance(world);
            }
            8 => {
                if *world.resource::<State<VnState>>().get() != VnState::Waiting {
                    return Ok(false);
                }
                let Some(Interaction::Choice { options }) = world
                    .resource::<VnEngine>()
                    .0
                    .current_interaction()
                    .map_err(|problem| problem.to_string())?
                else {
                    return Ok(false);
                };
                if options.len() != 2 || !options[0].contains("Partager") {
                    return Err(format!("Unexpected real narrative choice {options:?}"));
                }
                capture(world, "08_story_choice.png")?;
                world.send_event(PlayerInput::Choose(0));
                world.resource_mut::<Run>().choices += 1;
            }
            9 => {
                let Some(text) = dialogue(world)? else {
                    return Ok(false);
                };
                if !text.contains("voyagerons ensemble") {
                    return Err(format!(
                        "The sharing choice did not reach its actual branch: '{text}'"
                    ));
                }
                equal(world, "atlas_keeper_trust", Value::Int(2))?;
                check(world,"A real PlayerInput::Choose selects the sharing branch and changes keeper trust");
                advance(world);
            }
            10 => {
                let Some(text) = dialogue(world)? else {
                    return Ok(false);
                };
                if !text.contains("Prenez cette lanterne") {
                    return Err(format!(
                        "Expected the quest reward dialogue, found '{text}'"
                    ));
                }
                equal(world, "atlas_has_lantern", Value::Bool(true))?;
                keeper_layers(world, &["body", "coat", "smile", "lantern"])?;
                let character = &world.resource::<VnEngine>().0.state.layered.characters["keeper"];
                if character.attributes.get("outfit").map(String::as_str) != Some("travel")
                    || character.attributes.get("face").map(String::as_str) != Some("smile")
                    || character.attributes.get("accessory").map(String::as_str) != Some("lantern")
                {
                    return Err("Keeper independent attributes are inconsistent with the delivered quest reward".into());
                }
                capture(world, "09_keeper_smile_lantern.png")?;
                check(world,"Quest reward renders original coat, smile and lantern simultaneously, preserving independent saved attributes and aligned transparent layers");
                advance(world);
            }
            11 => {
                if !settled(world, 2) || !canvas_ready(world) {
                    return Ok(false);
                }
                equal(world, "atlas_open", Value::Bool(true))?;
                equal(world, "atlas_keeper_trust", Value::Int(2))?;
                let Value::List(quantities) = global(world, "atlas_quantities")? else {
                    return Err("Inventory quantities must remain a list".into());
                };
                if quantities.get(1) != Some(&Value::Int(1))
                    || quantities.get(4) != Some(&Value::Int(1))
                {
                    return Err("Harbour quest rewards were not retained in the inventory".into());
                }
                if world
                    .resource::<VnEngine>()
                    .0
                    .state
                    .sprites
                    .get("keeper")
                    .is_some_and(|sprite| sprite.visible)
                {
                    return Err("Keeper remains visible after returning to exploration".into());
                }
                capture(world, "10_map_after_quest.png")?;
                check(world,"Returning to exploration preserves quest rewards, native preferences and the modal menu state without a lingering keeper sprite");
            }
            _ => {
                for capture in &world.resource::<Run>().captures {
                    // Screenshot encoding/writing is asynchronous. Wait for the
                    // actual completed PNG instead of treating queued I/O as failure.
                    let path = world.resource::<Run>().directory.join(capture);
                    let bytes = match std::fs::read(&path) {
                        Ok(bytes) => bytes,
                        Err(problem) if problem.kind() == std::io::ErrorKind::NotFound => {
                            return Ok(false)
                        }
                        Err(problem) => {
                            return Err(format!("Cannot read capture '{capture}': {problem}"))
                        }
                    };
                    let Some(size) = png_size(&bytes) else {
                        return Ok(false);
                    };
                    if size[0] < 640 || size[1] < 360 {
                        return Err(format!(
                            "Native capture '{capture}' has unexpected size {size:?}"
                        ));
                    }
                }
                check(world,"Every requested capture is a completed native PNG; no runtime asset or script diagnostic occurred");
                write_report(world, "pass")?;
                world.resource_mut::<Run>().finishing = Some(now);
                return Ok(false);
            }
        }
        Ok(true)
    })();
    match result {
        Ok(true) => next(world, now),
        Ok(false) => {}
        Err(message) => fail(world, message, now),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evidence_png_header_requires_real_dimensions_and_signature() {
        assert_eq!(png_size(&[]), None);
        assert_eq!(png_size(&[0; 24]), None);
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend(1280u32.to_be_bytes());
        bytes.extend(720u32.to_be_bytes());
        assert_eq!(
            png_size(&bytes),
            None,
            "An incompletely written screenshot is not evidence"
        );
        bytes.extend(b"\0\0\0\0IEND\xae\x42\x60\x82");
        assert_eq!(png_size(&bytes), Some([1280, 720]));
    }
    #[test]
    fn numeric_checks_accept_only_real_numeric_globals() {
        assert_eq!(number(&Value::Int(1)), Some(1.0));
        assert_eq!(number(&Value::Float(1.15)), Some(1.15));
        assert_eq!(number(&Value::Str("1.15".into())), None);
        assert_eq!(number(&Value::Bool(true)), None);
    }
    #[test]
    fn isolated_atlas_ignores_hardware_but_preserves_driver_events() {
        use bevy::input::{
            keyboard::{Key, KeyboardInput},
            mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel},
            ButtonState,
        };
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<GamepadButton>>()
            .add_event::<KeyboardInput>()
            .add_event::<MouseButtonInput>()
            .add_event::<MouseWheel>()
            .add_event::<CursorMoved>()
            .add_event::<PlayerInput>();
        let own_window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let driver_point = Vec2::new(415.0, 281.0);
        app.insert_resource(Run {
            started: Instant::now(),
            phase: 3,
            entered: 0.0,
            directory: PathBuf::new(),
            images: Vec::new(),
            captures: Vec::new(),
            checks: Vec::new(),
            snapshots: Vec::new(),
            travel_pc: None,
            advances: 0,
            choices: 0,
            finishing: None,
            failure: None,
            pointer: Some((own_window, driver_point)),
        });
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut().send_event(KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key: Key::Escape,
            state: ButtonState::Pressed,
            window: own_window,
        });
        app.world_mut().send_event(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: own_window,
        });
        app.world_mut().send_event(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window: own_window,
        });
        app.world_mut().send_event(CursorMoved {
            window: own_window,
            position: Vec2::ONE,
            delta: None,
        });
        app.world_mut().send_event(PlayerInput::Advance);
        app.world_mut().send_event(PlayerInput::Choose(0));
        isolate_inputs(app.world_mut());
        assert!(!app
            .world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::Escape));
        assert!(!app
            .world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left));
        assert!(app.world().resource::<Events<KeyboardInput>>().is_empty());
        assert!(app
            .world()
            .resource::<Events<MouseButtonInput>>()
            .is_empty());
        assert!(app.world().resource::<Events<MouseWheel>>().is_empty());
        let mut reader = app.world().resource::<Events<CursorMoved>>().get_reader();
        let moves: Vec<_> = reader
            .read(app.world().resource::<Events<CursorMoved>>())
            .collect();
        assert_eq!(moves.len(), 1);
        assert_eq!(moves[0].position, driver_point);
        assert_eq!(
            app.world()
                .get::<Window>(own_window)
                .unwrap()
                .cursor_position(),
            Some(driver_point)
        );
        assert_eq!(
            app.world().resource::<Events<PlayerInput>>().len(),
            2,
            "Narrative driver events must not be filtered"
        );
    }
}
