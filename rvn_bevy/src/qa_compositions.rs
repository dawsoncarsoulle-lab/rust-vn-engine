//! Opt-in, isolated real-window QA for the layered-character example.
use crate::{
    components::{DialogueText, VnSprite},
    layered_characters::{CompositionRoot, LayerTarget},
    resources::{ScriptErrorMessage, TypewriterState, VnEngine, VnState},
    vn_command::PlayerInput,
};
use bevy::{
    app::AppExit, prelude::*, render::view::screenshot::ScreenshotManager, window::PrimaryWindow,
};

#[derive(Resource)]
struct Run {
    started: std::time::Instant,
    phase: usize,
    entered: f64,
    changed_from: Option<usize>,
    coat: Option<rvn_core::save::SaveData>,
    moving: Option<rvn_core::save::SaveData>,
}
fn flush(world: &mut World) {
    for command in world.resource_mut::<VnEngine>().0.renderer.take_pending() {
        world.send_event(command);
    }
}
fn advance(world: &mut World) {
    let pc = world.resource::<VnEngine>().0.state.pc;
    world
        .resource_mut::<VnEngine>()
        .0
        .advance_dialogue()
        .unwrap();
    world.resource_mut::<Run>().changed_from = Some(pc);
    world
        .resource_mut::<NextState<VnState>>()
        .set(VnState::Stepping);
}
fn save(world: &World) -> rvn_core::save::SaveData {
    rvn_core::save::SaveData::from_state(
        &world.resource::<VnEngine>().0.state,
        1,
        "Compositions QA".into(),
        "main.rvn".into(),
    )
}
fn capture(world: &mut World, name: &str) {
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world);
    let path = std::path::PathBuf::from(std::env::var_os("RVN_QA_OUTPUT").unwrap()).join(name);
    world
        .resource_mut::<ScreenshotManager>()
        .save_screenshot_to_disk(window, path)
        .unwrap();
}
fn layers(world: &mut World, expected: &[&str]) {
    let mut actual: Vec<_> = world
        .query::<&LayerTarget>()
        .iter(world)
        .filter(|target| target.character == "iris")
        .map(|target| target.layer.clone())
        .collect();
    actual.sort();
    let mut expected: Vec<_> = expected.iter().map(|id| id.to_string()).collect();
    expected.sort();
    assert_eq!(actual, expected);
    let roots = world
        .query::<(&VnSprite, &CompositionRoot, &GlobalTransform)>()
        .iter(world)
        .filter(|(sprite, _, _)| sprite.id == "iris")
        .count();
    assert_eq!(
        roots,
        usize::from(!expected.is_empty()),
        "Exactly one composition root must own its selected layers"
    );
}
pub(crate) fn drive(world: &mut World) {
    if !world.contains_resource::<Run>() {
        world.insert_resource(Run {
            started: std::time::Instant::now(),
            phase: 0,
            entered: 0.0,
            changed_from: None,
            coat: None,
            moving: None,
        });
    }
    let now = world.resource::<Run>().started.elapsed().as_secs_f64();
    assert!(
        now < 120.0,
        "Layered-character QA timed out at phase {}, pc {}",
        world.resource::<Run>().phase,
        world.resource::<VnEngine>().0.state.pc
    );
    let state = world.resource::<State<VnState>>().get().clone();
    assert_ne!(
        state,
        VnState::Error,
        "{}",
        world.resource::<ScriptErrorMessage>().0
    );
    if state == VnState::TitleScreen {
        if now > 1.0 {
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        return;
    }
    if state != VnState::Waiting || now - world.resource::<Run>().entered < 0.5 {
        return;
    }
    if world.resource::<Run>().changed_from == Some(world.resource::<VnEngine>().0.state.pc) {
        return;
    }
    let Ok(Some(rvn_core::Interaction::Dialogue { text: expected, .. })) =
        world.resource::<VnEngine>().0.current_interaction()
    else {
        return;
    };
    let text: Vec<_> = world
        .query_filtered::<&Text, With<DialogueText>>()
        .iter(world)
        .flat_map(|text| text.sections.iter().map(|section| section.value.clone()))
        .collect();
    if world.resource::<TypewriterState>().typing {
        world.send_event(PlayerInput::SkipTypewriter);
        return;
    }
    assert_eq!(
        text.concat(),
        expected,
        "A QA phase must not certify a blank or stale dialogue"
    );
    if world
        .query::<&Handle<Image>>()
        .iter(world)
        .any(|handle| world.resource::<Assets<Image>>().get(handle).is_none())
    {
        return;
    }
    let phase = world.resource::<Run>().phase;
    match phase {
        0 => {
            layers(world, &["body", "shirt", "neutral"]);
            capture(world, "01_defaults.png");
            advance(world);
        }
        1 => {
            layers(world, &["body", "coat", "neutral"]);
            let data = save(world);
            world.resource_mut::<Run>().coat = Some(data);
            capture(world, "02_coat.png");
            advance(world);
        }
        2 => {
            layers(world, &["body", "coat", "happy", "badge"]);
            assert_eq!(world.resource::<VnEngine>().0.state.motions.tracks.len(), 2);
            let data = save(world);
            world.resource_mut::<Run>().moving = Some(data);
            capture(world, "03_happy_and_motion.png");
        }
        3 => {
            let data = world.resource::<Run>().coat.clone().unwrap();
            world.resource_mut::<VnEngine>().0.load_data(data).unwrap();
            flush(world);
        }
        4 => {
            layers(world, &["body", "coat", "neutral"]);
            capture(world, "04_loaded_coat.png");
        }
        5 => {
            let data = world.resource::<Run>().moving.clone().unwrap();
            world.resource_mut::<VnEngine>().0.load_data(data).unwrap();
            flush(world);
        }
        6 => {
            layers(world, &["body", "coat", "happy", "badge"]);
            capture(world, "05_loaded_motion.png");
            advance(world);
        }
        7 => {
            assert!(world
                .resource::<VnEngine>()
                .0
                .state
                .motions
                .waiting
                .is_none());
            assert!(
                !world.resource::<VnEngine>().0.state.motions.tracks["layer:iris/badge"].running
            );
            capture(world, "06_after_wait.png");
            advance(world);
        }
        8 => {
            layers(world, &["body", "shirt", "happy"]);
            assert!(world
                .resource::<VnEngine>()
                .0
                .state
                .motions
                .tracks
                .is_empty());
            capture(world, "07_independent_attributes.png");
            advance(world);
        }
        9 => {
            layers(world, &[]);
            capture(world, "08_hidden.png");
            assert!(world.resource_mut::<VnEngine>().0.rollback());
            assert!(world.resource_mut::<VnEngine>().0.rollback());
            flush(world);
        }
        10 => {
            layers(world, &["body", "shirt", "happy"]);
            capture(world, "09_rollback.png");
            let path = std::path::PathBuf::from(std::env::var_os("RVN_QA_OUTPUT").unwrap())
                .join("result.json");
            std::fs::write(path,serde_json::to_vec_pretty(&serde_json::json!({"result":"pass","scenario":"native layered characters: real assets, outfit/expression independence, conditional layer, whole/layer motion, wait, save/load, disappearance and rollback; rendered dialogue verified"})).unwrap()).unwrap();
        }
        _ => {
            world.send_event(AppExit::Success);
            return;
        }
    }
    let mut run = world.resource_mut::<Run>();
    run.phase += 1;
    run.entered = now;
    if !matches!(phase, 0 | 1 | 6 | 7 | 8) {
        run.changed_from = None;
    }
}
