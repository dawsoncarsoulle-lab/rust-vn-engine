//! Opt-in QA for the owned examples/custom-components project. No normal game
//! or user editor is affected. Inputs traverse the actual Bevy event systems;
//! captures are taken from the rendered window, not a synthetic bitmap.
use super::*;
use bevy::{app::AppExit, render::view::screenshot::ScreenshotManager, window::PrimaryWindow};

#[derive(Resource)]
struct Run {
    started: std::time::Instant,
    step: usize,
    entered: f64,
    saved: Option<rvn_core::save::SaveData>,
    initial_entity: Option<Entity>,
    number: f32,
    elapsed: f64,
    keyboard_count: i64,
    checks: Vec<&'static str>,
    shortcut: usize,
}
fn local_state<'a>(world: &'a World, id: &str) -> &'a std::collections::BTreeMap<String, Value> {
    let state = &world
        .resource::<VnEngine>()
        .0
        .state
        .ui
        .screens
        .iter()
        .find(|screen| screen.name == "custom")
        .unwrap()
        .canvas_states[id]
        .state;
    let Value::Dict(state) = state else {
        panic!("Canvas local state must be a dictionary")
    };
    state
}
fn number(value: &Value) -> f32 {
    match value {
        Value::Float(number) => *number,
        Value::Int(number) => *number as f32,
        _ => panic!("Expected numeric state"),
    }
}
fn value(world: &World, id: &str) -> f32 {
    number(&local_state(world, id)["value"])
}
fn elapsed(world: &World) -> f64 {
    world
        .resource::<VnEngine>()
        .0
        .state
        .ui
        .screens
        .iter()
        .find(|screen| screen.name == "custom")
        .unwrap()
        .canvas_states["first"]
        .elapsed
}
fn global<'a>(world: &'a World, name: &str) -> &'a Value {
    &world.resource::<VnEngine>().0.state.vars[name]
}
fn window(world: &mut World) -> Entity {
    world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
}
fn screen_point(world: &mut World, id: &str, local: Vec2) -> Vec2 {
    let entity = canvas_entity(world, "custom", id).unwrap();
    let surface = world.get::<CanvasSurface>(entity).unwrap();
    let size = world.get::<Node>(entity).unwrap().size();
    world
        .get::<GlobalTransform>(entity)
        .unwrap()
        .transform_point(((local / surface.extent - Vec2::splat(0.5)) * size).extend(0.0))
        .truncate()
}
fn move_pointer(world: &mut World, point: Vec2) {
    let window = window(world);
    world
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(point));
    world.send_event(CursorMoved {
        window,
        position: point,
        delta: None,
    });
}
fn mouse(world: &mut World, state: ButtonState) {
    let window = window(world);
    world.send_event(MouseButtonInput {
        window,
        button: MouseButton::Left,
        state,
    });
}
fn key(world: &mut World, state: ButtonState) {
    let window = window(world);
    world.send_event(KeyboardInput {
        window,
        key_code: KeyCode::ArrowLeft,
        logical_key: Key::ArrowLeft,
        state,
    });
}
fn shortcut(world: &mut World, code: KeyCode, state: ButtonState) {
    let window = window(world);
    world.send_event(KeyboardInput {
        window,
        key_code: code,
        logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        state,
    });
}
fn screenshot(world: &mut World, name: &str) {
    let directory = std::path::PathBuf::from(std::env::var_os("RVN_QA_OUTPUT").unwrap());
    let window = window(world);
    world
        .resource_mut::<ScreenshotManager>()
        .save_screenshot_to_disk(window, directory.join(name))
        .unwrap();
}
fn flush(world: &mut World) {
    for command in world.resource_mut::<VnEngine>().0.renderer.take_pending() {
        world.send_event(command);
    }
}
fn check(world: &mut World, message: &'static str) {
    world.resource_mut::<Run>().checks.push(message);
}

pub(super) fn drive(world: &mut World) {
    if !world.contains_resource::<Run>() {
        world.insert_resource(Run {
            started: std::time::Instant::now(),
            step: 0,
            entered: 0.0,
            saved: None,
            initial_entity: None,
            number: 0.0,
            elapsed: 0.0,
            keyboard_count: 0,
            checks: Vec::new(),
            shortcut: 0,
        });
    }
    let now = world.resource::<Run>().started.elapsed().as_secs_f64();
    let step = world.resource::<Run>().step;
    assert!(
        now < 120.0,
        "Canvas QA timed out at phase {step}, error={}",
        world.resource::<ScriptErrorMessage>().0
    );
    let state = world.resource::<State<VnState>>().get().clone();
    if std::env::var_os("RVN_QA_CUSTOM_CANVAS_NEGATIVE").is_some() {
        if step == 0 && state == VnState::Error {
            let message = &world.resource::<ScriptErrorMessage>().0;
            assert!(
                message.contains("qa-deliberately-missing.png")
                    && message.contains("canvas 'first'"),
                "Unexpected resource diagnostic: {message}"
            );
            screenshot(world, "missing_resource_diagnostic.png");
            let directory = std::path::PathBuf::from(std::env::var_os("RVN_QA_OUTPUT").unwrap());
            std::fs::write(directory.join("negative-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"result":"pass","platform":"Linux actual Bevy UI window","checks":["Authored missing canvas image produces a named visible runtime diagnostic, never silent success"]})).unwrap()).unwrap();
            let mut run = world.resource_mut::<Run>();
            run.step = 1;
            run.entered = now;
        } else if step == 1 && now - world.resource::<Run>().entered > 0.55 {
            world.send_event(AppExit::Success);
        }
        return;
    }
    if step < 20 {
        assert_ne!(
            state,
            VnState::Error,
            "{}",
            world.resource::<ScriptErrorMessage>().0
        );
    }
    let window = window(world);
    // This isolated synthetic-input QA does not steal OS focus from the user.
    // Only its own Window resource is enabled; phase10 explicitly tests pause.
    if step != 10 && step < 20 {
        world.get_mut::<Window>(window).unwrap().focused = true;
    }
    if state == VnState::TitleScreen {
        if now > 1.5 {
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        return;
    }
    if now - world.resource::<Run>().entered < 0.55 {
        return;
    }
    if step < 8
        && state != VnState::Waiting
        && !(step == 7 && world.resource::<Run>().shortcut >= 2)
    {
        return;
    }
    if step == 0 {
        let surfaces: Vec<_> = world
            .query::<(Entity, &CanvasSurface)>()
            .iter(world)
            .map(|(entity, surface)| (entity, surface.clone()))
            .collect();
        if surfaces.len() != 2 || world.query::<&CanvasLeaf>().iter(world).count() < 14 {
            return;
        }
        if world
            .resource::<CanvasCache>()
            .materials
            .values()
            .any(|handle| {
                world
                    .resource::<Assets<CanvasMaterial>>()
                    .get(handle)
                    .is_none_or(|material| {
                        !world
                            .resource::<Assets<Image>>()
                            .contains(&material.texture)
                    })
            })
        {
            return;
        }
        for (entity, _) in surfaces {
            assert!(world.get::<Node>(entity).unwrap().size().min_element() > 100.0);
            assert!(world.get::<ViewVisibility>(entity).unwrap().get());
        }
        assert!((value(world, "first") - 0.25).abs() < 1e-5);
        assert!((value(world, "second") - 0.75).abs() < 1e-5);
        assert!(elapsed(world) > 0.0);
        assert!(number(&local_state(world, "first")["updates"]) > 0.0);
        let first = canvas_entity(world, "custom", "first");
        world.resource_mut::<Run>().initial_entity = first;
        screenshot(world, "01_canvas_primitives.png");
        check(world,"Actual canvas UI, fonts/images, concave polygon, affine group clips and independent initial states rendered");
    }
    match step {
        0 => {}
        1 => {
            assert_eq!(
                canvas_entity(world, "custom", "first"),
                world.resource::<Run>().initial_entity,
                "Simulation-only frames must not recreate host UI entities"
            );
            check(
                world,
                "Tick frames preserve host UI entities and retain focus/scroll identity",
            );
            let point = screen_point(world, "first", Vec2::new(580.0, 108.0));
            move_pointer(world, point);
            mouse(world, ButtonState::Pressed);
        }
        2 => {
            assert!(value(world, "first") > 0.7);
            assert!((value(world, "second") - 0.75).abs() < 1e-5);
            assert_eq!(
                global(world, "custom_last_event"),
                &Value::Str("pointer_down".into())
            );
            assert_eq!(
                global(world, "custom_last_hit"),
                &Value::Str("track".into())
            );
            assert!(world
                .resource::<PointerState>()
                .captures
                .contains_key("mouse"));
            let point = screen_point(world, "first", Vec2::new(840.0, 108.0));
            move_pointer(world, point);
        }
        3 => {
            assert!((value(world, "first") - 1.0).abs() < 1e-5);
            assert!((value(world, "second") - 0.75).abs() < 1e-5);
            assert_eq!(
                global(world, "custom_last_event"),
                &Value::Str("pointer_move".into())
            );
            check(world,"Native pointer down/move use local geometry; capture outside canvas does not modify the peer");
            screenshot(world, "02_captured_drag.png");
            mouse(world, ButtonState::Released);
        }
        4 => {
            assert_eq!(local_state(world, "first")["dragging"], Value::Bool(false));
            assert!(world.resource::<PointerState>().captures.is_empty());
            assert_eq!(
                global(world, "custom_last_event"),
                &Value::Str("pointer_up".into())
            );
            let point = screen_point(world, "first", Vec2::new(580.0, 108.0));
            move_pointer(world, point);
            world.send_event(MouseWheel {
                window,
                x: 0.0,
                y: -1.0,
                unit: MouseScrollUnit::Line,
            });
        }
        5 => {
            assert!((value(world, "first") - 0.95).abs() < 1e-5);
            assert_eq!(global(world, "custom_last_wheel"), &Value::Float(-36.0));
            key(world, ButtonState::Pressed);
            check(
                world,
                "Pointer up releases capture and native wheel reports normalized local input",
            );
        }
        6 => {
            assert!((value(world, "first") - 0.9).abs() < 1e-5);
            assert_eq!(
                global(world, "custom_last_key"),
                &Value::Str("ArrowLeft".into())
            );
            let number = value(world, "first");
            let count = number_i64(global(world, "custom_event_count"));
            world.resource_mut::<Run>().number = number;
            world.resource_mut::<Run>().keyboard_count = count;
            key(world, ButtonState::Released);
        }
        7 => {
            let shortcut_phase = world.resource::<Run>().shortcut;
            if shortcut_phase != 0 {
                match shortcut_phase {
                    1 => {
                        let paths = world.resource::<crate::project_paths::ProjectPaths>();
                        assert!(paths.saves.join("quicksave.json").exists(),"F5 must reach the real quick-save system while the Canvas retains keyboard focus");
                        assert_eq!(
                            global(world, "custom_last_key"),
                            &Value::Str("ArrowLeft".into())
                        );
                        assert!(!world.resource::<Screens>().keyboard_consumed);
                        shortcut(world, KeyCode::F5, ButtonState::Released);
                        shortcut(world, KeyCode::Escape, ButtonState::Pressed);
                    }
                    2 => {
                        assert_eq!(state, VnState::Menu);
                        shortcut(world, KeyCode::Escape, ButtonState::Released);
                        let elapsed = elapsed(world);
                        world.resource_mut::<Run>().elapsed = elapsed;
                    }
                    3 => {
                        assert_eq!(elapsed(world), world.resource::<Run>().elapsed);
                        world
                            .resource_mut::<NextState<VnState>>()
                            .set(VnState::Waiting);
                    }
                    4 => {
                        assert_eq!(state, VnState::Waiting);
                        shortcut(world, KeyCode::F6, ButtonState::Pressed);
                    }
                    5 => {
                        assert_eq!(state, VnState::Menu);
                        let mut confirmation =
                            world.resource_mut::<crate::systems::save_menu::SaveConfirmation>();
                        assert_eq!(
                            confirmation.pending,
                            Some((0, crate::systems::save_menu::SaveMenuMode::Load))
                        );
                        confirmation.pending = None;
                        confirmation.quick_return = None;
                        shortcut(world, KeyCode::F6, ButtonState::Released);
                        let elapsed = elapsed(world);
                        world.resource_mut::<Run>().elapsed = elapsed;
                        check(world,"Focused Canvas reserves F5/F6/Escape: real quick save, load confirmation and game menu remain accessible");
                    }
                    _ => unreachable!(),
                }
                if shortcut_phase < 5 {
                    let mut run = world.resource_mut::<Run>();
                    run.shortcut += 1;
                    run.entered = now;
                    return;
                }
            } else {
                assert_eq!(value(world, "first"), world.resource::<Run>().number);
                assert!(
                    number_i64(global(world, "custom_event_count"))
                        > world.resource::<Run>().keyboard_count
                );
                check(world,"Native keyboard down and up are both delivered; keyup does not double-apply the value change");
                let engine = &world.resource::<VnEngine>().0;
                let saved = rvn_core::save::SaveData::from_state(
                    &engine.state,
                    1,
                    "Custom canvas QA".into(),
                    "main.rvn".into(),
                );
                let directory =
                    std::path::PathBuf::from(std::env::var_os("RVN_QA_OUTPUT").unwrap());
                std::fs::write(
                    directory.join("canvas-save.json"),
                    serde_json::to_vec_pretty(&saved).unwrap(),
                )
                .unwrap();
                world.resource_mut::<Run>().saved = Some(saved);
                let elapsed = elapsed(world);
                world.resource_mut::<Run>().elapsed = elapsed;
                assert!(
                    !world
                        .resource::<crate::project_paths::ProjectPaths>()
                        .saves
                        .join("quicksave.json")
                        .exists(),
                    "QA must use a fresh isolated example copy"
                );
                shortcut(world, KeyCode::F5, ButtonState::Pressed);
                let mut run = world.resource_mut::<Run>();
                run.shortcut = 1;
                run.entered = now;
                return;
            }
        }
        8 => {
            assert_eq!(state, VnState::Menu);
            assert_eq!(elapsed(world), world.resource::<Run>().elapsed);
            check(world, "Game menu pauses the saved canvas simulation clock");
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Waiting);
        }
        9 => {
            assert_eq!(state, VnState::Waiting);
            let elapsed = elapsed(world);
            world.resource_mut::<Run>().elapsed = elapsed;
            world.get_mut::<Window>(window).unwrap().focused = false;
            world.send_event(WindowFocused {
                window,
                focused: false,
            });
        }
        10 => {
            assert_eq!(elapsed(world), world.resource::<Run>().elapsed);
            check(
                world,
                "Unfocused window pauses simulation without consuming wall-clock time",
            );
            world.get_mut::<Window>(window).unwrap().focused = true;
            world.send_event(WindowFocused {
                window,
                focused: true,
            });
        }
        11 => {
            let point = screen_point(world, "first", Vec2::new(320.0, 108.0));
            move_pointer(world, point);
            mouse(world, ButtonState::Pressed);
        }
        12 => {
            assert!(world
                .resource::<PointerState>()
                .captures
                .contains_key("mouse"));
            let saved = world.resource::<Run>().saved.clone().unwrap();
            let ui = saved.ui.clone();
            let random = saved.random.clone();
            world.resource_mut::<VnEngine>().0.load_data(saved).unwrap();
            assert_eq!(world.resource::<VnEngine>().0.state.ui, ui);
            assert_eq!(world.resource::<VnEngine>().0.state.random, random);
            flush(world);
            mouse(world, ButtonState::Released);
        }
        13 => {
            assert!(world.resource::<PointerState>().captures.is_empty());
            assert!((value(world, "first") - 0.9).abs() < 1e-5);
            assert!((value(world, "second") - 0.75).abs() < 1e-5);
            screenshot(world, "03_restored_canvas.png");
            check(world,"JSON save/load restores both instance dictionaries, clock and random state; capture is not restored");
            world
                .resource_mut::<VnEngine>()
                .0
                .advance_dialogue()
                .unwrap();
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        14 => {
            if state != VnState::Waiting {
                return;
            }
            let point = screen_point(world, "second", Vec2::new(160.0, 108.0));
            move_pointer(world, point);
            mouse(world, ButtonState::Pressed);
        }
        15 => {
            assert!(value(world, "second") < 0.3);
            mouse(world, ButtonState::Released);
        }
        16 => {
            // UI down/up changes each form a history entry, just like ordinary
            // controls. Walk the two interaction checkpoints back to pre-drag.
            let mut restored = false;
            for _ in 0..8 {
                assert!(world.resource_mut::<VnEngine>().0.rollback());
                assert!((value(world, "first") - 0.9).abs() < 1e-5);
                if (value(world, "second") - 0.75).abs() < 1e-5
                    && local_state(world, "second")["dragging"] == Value::Bool(false)
                {
                    restored = true;
                    break;
                }
            }
            assert!(
                restored,
                "Successive rollbacks must reach the peer's pre-drag dictionary"
            );
            flush(world);
        }
        17 => {
            assert!(world.resource::<PointerState>().captures.is_empty());
            screenshot(world, "04_rollback_canvas.png");
            check(world,"Successive interaction rollbacks restore independent pre-drag values and invalidate transient capture");
            let point = screen_point(world, "first", Vec2::new(380.0, 108.0));
            move_pointer(world, point);
            mouse(world, ButtonState::Pressed);
        }
        18 => {
            assert!(world
                .resource::<PointerState>()
                .captures
                .contains_key("mouse"));
            world.send_event(WindowFocused {
                window,
                focused: false,
            });
        }
        19 => {
            assert!(world.resource::<PointerState>().captures.is_empty());
            assert_eq!(local_state(world, "first")["dragging"], Value::Bool(false));
            check(world,"Focus loss delivers pointer_cancel and clears drag/capture without a synthetic click");
            let directory = std::path::PathBuf::from(std::env::var_os("RVN_QA_OUTPUT").unwrap());
            let checks = world.resource::<Run>().checks.clone();
            std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"result":"pass","platform":"Linux actual Bevy UI window","checks":checks})).unwrap()).unwrap();
        }
        _ => {
            world.send_event(AppExit::Success);
            return;
        }
    }
    let mut run = world.resource_mut::<Run>();
    run.step += 1;
    run.entered = now;
}
fn number_i64(value: &Value) -> i64 {
    match value {
        Value::Int(value) => *value,
        _ => panic!("Expected integer"),
    }
}
