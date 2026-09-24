//! Explicit opt-in visual QA through the same PlayerInput events as the UI.
//! RVN_QA_OUTPUT must name an existing directory. Normal launches do nothing.
use crate::{
    resources::{VnEngine, VnState},
    vn_command::PlayerInput,
};
use bevy::{
    app::AppExit, prelude::*, render::view::screenshot::ScreenshotManager, window::PrimaryWindow,
};
use std::{collections::BTreeSet, path::PathBuf};
#[path = "qa_menu_designs.rs"]
mod design_capture;

#[derive(Default)]
pub(crate) struct QaPlugin;
impl Plugin for QaPlugin {
    fn build(&self, app: &mut App) {
        let Some(dir) = std::env::var_os("RVN_QA_OUTPUT").map(PathBuf::from) else {
            return;
        };
        assert!(dir.is_dir(), "RVN_QA_OUTPUT must be an existing directory");
        // Synthetic pointer events must receive a full UI frame even when the
        // compositor has not focused this isolated verification window.
        app.insert_resource(bevy::winit::WinitSettings::game());
        app.init_resource::<QaPointer>().add_systems(
            PreUpdate,
            apply_qa_pointer
                .after(bevy::input::InputSystem)
                .before(bevy::ui::UiSystem::Focus),
        );
        if std::env::var_os("RVN_QA_MENU_DESIGN").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, design_capture::drive);
            return;
        }
        if std::env::var_os("RVN_QA_LOCAL_CONTROLS").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, local_controls_drive);
            return;
        }
        if std::env::var_os("RVN_QA_AUTO_HEIGHT").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, auto_height_drive);
            return;
        }
        if std::env::var_os("RVN_QA_TEXT_STATES").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, text_states_drive);
            return;
        }
        if std::env::var_os("RVN_QA_RESUME_THUMBNAILS").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, resume_thumbnail_drive);
            return;
        }
        if std::env::var_os("RVN_QA_PAGINATION").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, pagination_drive);
            return;
        }
        if std::env::var_os("RVN_QA_NAVIGATION_LOOP").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, navigation_loop_drive);
            return;
        }
        if std::env::var_os("RVN_QA_CONDITIONAL_CARDS").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, conditional_cards_drive);
            return;
        }
        if std::env::var_os("RVN_QA_CONTROLLER_CHOICES").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, controller_choices_drive);
            return;
        }
        if std::env::var_os("RVN_QA_DISABLED_CHOICES").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, disabled_choices_drive);
            return;
        }
        if std::env::var_os("RVN_QA_DISABLED_CONTROL").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, disabled_control_drive);
            return;
        }
        if std::env::var_os("RVN_QA_CONTROLLER_FOCUS").is_some() {
            app.insert_resource(Qa {
                dir,
                started: std::time::Instant::now(),
                pc: usize::MAX,
                entered: 0.0,
                captured: BTreeSet::new(),
                choices: 0,
                finishing: None,
                menu_step: 0,
                menus: false,
                map_target: None,
                mouse_down: false,
                map_clicked: None,
                slider_step: 0,
            });
            app.add_systems(Update, controller_focus_drive);
            return;
        }
        if std::env::var_os("RVN_QA_QUICK_ACTIONS").is_some() {
            app.add_systems(Update, approve_quickload);
        }
        app.insert_resource(Qa {
            dir,
            started: std::time::Instant::now(),
            pc: usize::MAX,
            entered: 0.0,
            captured: BTreeSet::new(),
            choices: 0,
            finishing: None,
            menu_step: 0,
            menus: std::env::var_os("RVN_QA_MENU_ACTIONS").is_some(),
            map_target: std::env::var("RVN_QA_MAP_TARGET")
                .ok()
                .and_then(|v| v.parse().ok()),
            mouse_down: false,
            map_clicked: None,
            slider_step: 0,
        });
        if std::env::var_os("RVN_QA_LONG_DIALOGUE").is_some() {
            app.add_systems(Update, long_dialogue_drive);
        } else if std::env::var_os("RVN_QA_LONG_DROPDOWN").is_some() {
            app.add_systems(Update, long_dropdown_drive);
        } else if std::env::var_os("RVN_QA_POINTER").is_some() {
            app.add_systems(Update, pointer_drive);
        } else if std::env::var_os("RVN_QA_GENERAL_CONFIRM").is_some() {
            app.add_systems(
                Update,
                general_confirm_drive.after(crate::systems::player_input_system),
            );
        } else if std::env::var_os("RVN_QA_SAVE_CARDS").is_some() {
            app.add_systems(
                Update,
                slots_drive.after(crate::systems::save_menu_interaction_system),
            );
        } else if std::env::var_os("RVN_QA_ANIMATION").is_some() {
            app.add_systems(Update, animation_drive);
        } else if std::env::var_os("RVN_QA_DROPDOWN").is_some() {
            app.add_systems(Update, dropdown_drive);
        } else if std::env::var_os("RVN_QA_SCROLL").is_some() {
            app.add_systems(Update, scroll_drive);
        } else if std::env::var_os("RVN_QA_CONFIRMATION").is_some() {
            app.add_systems(
                Update,
                confirm_drive
                    .after(crate::systems::save_menu_interaction_system)
                    .after(crate::systems::player_input_system),
            );
        } else {
            app.add_systems(Update, drive);
        }
    }
}
fn local_controls_drive(
    qa: Res<Qa>,
    engine: Res<VnEngine>,
    menus: Res<crate::menu_documents::Menus>,
    settings: Res<crate::systems::settings_menu::Settings>,
    nodes: Query<(&crate::menu_documents::VisualNode, &Node, &GlobalTransform)>,
    mut pointer: ResMut<QaPointer>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize, String, f32)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 25.0 {
        panic!("Local controls timeout at {}", phase.0);
    }
    if now < 1.5 || now - phase.1 < 0.5 {
        return;
    }
    let position = |id: &str| {
        nodes
            .iter()
            .find(|(n, _, _)| n.id == id)
            .map(|(_, n, t)| (n.size(), t.translation().truncate()))
    };
    match phase.0 {
        0 => {
            phase.2 = engine.0.state.pc;
            phase.3 = settings.language.clone();
            phase.4 = settings.music_volume;
            let Some((_, p)) = position("local_0") else {
                return;
            };
            pointer.position = Some(p);
        }
        1 | 5 | 8 => pointer.buttons.push(bevy::input::ButtonState::Pressed),
        2 | 6 | 9 => pointer.buttons.push(bevy::input::ButtonState::Released),
        3 => {
            assert_eq!(
                menus.session.variables.get("ui.control_0"),
                Some(&true.into())
            );
            assert_eq!(menus.session.variables.get("changed_0"), Some(&true.into()));
        }
        4 => {
            let Some((size, p)) = position("local_1") else {
                return;
            };
            pointer.position = Some(Vec2::new(
                p.x - size.x / 2.0 + 12.0 + (size.x - 24.0) * 0.75,
                p.y,
            ));
        }
        7 => {
            assert_eq!(
                menus
                    .session
                    .variables
                    .get("ui.control_1")
                    .and_then(|v| v.as_f64()),
                Some(75.0)
            );
            assert_eq!(menus.session.variables.get("changed_1"), Some(&true.into()));
            let Some((_, p)) = position("local_2") else {
                return;
            };
            pointer.position = Some(p);
        }
        10 => pointer.keys.push((KeyCode::ArrowDown, true)),
        11 => pointer.keys.push((KeyCode::ArrowDown, false)),
        12 => pointer.keys.push((KeyCode::Enter, true)),
        13 => pointer.keys.push((KeyCode::Enter, false)),
        14 => {
            for i in 0..3 {
                assert!(
                    !menus
                        .session
                        .variables
                        .contains_key(&format!("duplicate_{i}")),
                    "A control dispatched ValueChanged twice for one unchanged value"
                );
            }
            assert_eq!(
                menus.session.variables.get("ui.control_2"),
                Some(&"Papier".into())
            );
            assert_eq!(menus.session.variables.get("changed_2"), Some(&true.into()));
            assert_eq!(engine.0.state.pc, phase.2);
            assert_eq!(settings.language, phase.3);
            assert_eq!(settings.music_volume, phase.4);
            assert_eq!(
                menus
                    .document()
                    .unwrap()
                    .find_element(0, "local_0")
                    .unwrap()
                    .local_control
                    .as_ref()
                    .unwrap()
                    .initial,
                false
            );
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("local-controls.png"))
                .unwrap();
            std::fs::write(qa.dir.join("local-controls.txt"),"Checkbox, draggable slider and ordered keyboard selector update UI-only variables and ValueChanged graphs. Source, settings and narrative position unchanged.").unwrap();
        }
        15 => {
            exit.send(AppExit::Success);
        }
        _ => return,
    }
    phase.0 += 1;
    phase.1 = now;
}
fn auto_height_drive(
    qa: Res<Qa>,
    engine: Res<VnEngine>,
    mut menus: ResMut<crate::menu_documents::Menus>,
    nodes: Query<(&crate::menu_documents::VisualNode, &Node, &GlobalTransform)>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize, f32)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 15.0 {
        panic!("Automatic height timeout at {}", phase.0);
    }
    if now < 2.0 || now - phase.1 < 0.8 {
        return;
    }
    let rect = |id: &str| {
        nodes
            .iter()
            .find(|(n, _, _)| n.id == id)
            .map(|(_, n, t)| (n.size(), t.translation()))
    };
    let Some((size, pos)) = rect("long_button") else {
        return;
    };
    let Some((after, after_pos)) = rect("after_button") else {
        return;
    };
    assert!(
        after_pos.y - after.y / 2.0 >= pos.y + size.y / 2.0,
        "Following button overlaps the measured text"
    );
    match phase.0 {
        0 => {
            assert!(size.y > after.y * 2.0, "Long text did not expand");
            phase.2 = engine.0.state.pc;
            phase.3 = size.y;
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("auto-height-long.png"))
                .unwrap();
        }
        1 => {
            let page = menus.document().unwrap().pages[0].id.clone();
            menus.session.presentation.insert(
                (page, "long_button".into()),
                rvn_ui::ElementState {
                    text: Some("Court".into()),
                    ..default()
                },
            );
        }
        2 => {
            assert!(size.y < phase.3 / 2.0, "Short text did not shrink");
            assert_eq!(engine.0.state.pc, phase.2);
            let doc = menus.document().unwrap();
            assert!(doc.find_element(0, "long_button").unwrap().text.len() > 100);
            assert_eq!(doc.find_element(0, "long_button").unwrap().rect[3], 50.0);
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("auto-height-short.png"))
                .unwrap();
            std::fs::write(qa.dir.join("auto-height.txt"),"Long labels expand; following controls move; changed labels shrink to minimum height; source and story remain unchanged.").unwrap();
        }
        3 => {
            exit.send(AppExit::Success);
        }
        _ => return,
    }
    phase.0 += 1;
    phase.1 = now;
}
fn pagination_drive(
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    mut save: ResMut<crate::systems::save_menu::SaveMenuState>,
    texts: Query<&Text>,
    nodes: Query<(
        &crate::menu_documents::VisualNode,
        &GlobalTransform,
        Option<&Interaction>,
    )>,
    slots: Query<(
        &crate::systems::save_menu::SaveSlotButton,
        &Node,
        &GlobalTransform,
    )>,
    mut pointer: ResMut<QaPointer>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize)>,
) {
    use crate::systems::save_menu::{SaveMenuMode as Mode, SaveMenuOrigin as Origin};
    let now = qa.started.elapsed().as_secs_f64();
    if now > 25.0 {
        panic!("Pagination timeout at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.5 {
        return;
    }
    let page = |n: usize| {
        texts
            .iter()
            .flat_map(|t| &t.sections)
            .any(|s| s.value == format!("Page : {n}"))
    };
    match phase.0 {
        0 => next.set(VnState::Stepping),
        1 if *state.get() == VnState::Waiting => {
            phase.2 = engine.0.state.pc;
            save.open(Mode::Save, Origin::InGame);
            next.set(VnState::Menu);
        }
        2 | 5 | 8 => {
            let (expected, id) = match phase.0 {
                2 => (1, "next"),
                5 => (2, "three"),
                _ => (3, "previous"),
            };
            assert!(page(expected));
            for slot in ((expected - 1) * 6 + 1)..=(expected * 6) {
                assert!(
                    texts
                        .iter()
                        .flat_map(|t| &t.sections)
                        .any(|s| s.value == slot.to_string()),
                    "Slot {slot} is not displayed by its bound field"
                );
            }
            let Some((_, position, _)) = nodes.iter().find(|(n, _, _)| n.id == id) else {
                return;
            };
            pointer.position = Some(position.translation().truncate());
        }
        3 | 6 | 9 => {
            let id = match phase.0 {
                3 => "next",
                6 => "three",
                _ => "previous",
            };
            if !nodes
                .iter()
                .any(|(n, _, i)| n.id == id && i == Some(&Interaction::Hovered))
            {
                return;
            }
            pointer.buttons.push(bevy::input::ButtonState::Pressed);
        }
        4 | 7 | 10 => pointer.buttons.push(bevy::input::ButtonState::Released),
        11 => {
            assert!(page(2));
            let mut visible: Vec<_> = slots
                .iter()
                .filter(|(_, n, _)| n.size().x > 0.0 && n.size().y > 0.0)
                .map(|(s, _, _)| s.0)
                .collect();
            visible.sort();
            assert_eq!(visible, (7..=12).collect::<Vec<_>>());
            let position = |slot| {
                slots
                    .iter()
                    .find(|(s, _, _)| s.0 == slot)
                    .unwrap()
                    .2
                    .translation()
            };
            let a = position(7);
            let b = position(8);
            if std::env::var_os("RVN_QA_COLUMN_ORDER").is_some() {
                assert!(
                    (a.x - b.x).abs() < 1.0 && b.y > a.y,
                    "Column order must put slot 8 below 7"
                );
            } else {
                assert!(
                    b.x > a.x && (a.y - b.y).abs() < 1.0,
                    "Row order must put slot 8 beside 7"
                );
            }
            assert_eq!(engine.0.state.pc, phase.2);
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("pagination.png"))
                .unwrap();
        }
        12 => {
            save.open(Mode::Load, Origin::InGame);
        }
        13 => {
            assert!(page(1), "Save and Load pages share pagination state");
            save.open(Mode::Save, Origin::InGame);
        }
        14 => {
            assert!(page(2));
            assert_eq!(engine.0.state.pc, phase.2);
            std::fs::write(qa.dir.join("pagination.txt"),"Custom buttons select pages 1, 2, 3 and back; click graph replaces shortcut, slots 7–12 match page 2, Save/Load preserve independent pages and the story is unchanged.").unwrap();
        }
        15 => {
            exit.send(AppExit::Success);
        }
        _ => return,
    }
    phase.0 += 1;
    phase.1 = now;
}
fn navigation_loop_drive(
    qa: Res<Qa>,
    menus: Res<crate::menu_documents::Menus>,
    engine: Res<VnEngine>,
    state: Res<State<VnState>>,
    mut exit: EventWriter<AppExit>,
    mut captured: Local<Option<usize>>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now < 2.0 {
        return;
    }
    let error = menus
        .session
        .last_error
        .as_ref()
        .expect("The automatic page cycle must produce an editor diagnostic");
    assert_eq!(error.graph, "navigation_a_corriger");
    assert_eq!(error.node, Some(12));
    assert!(error.message.contains("16 transitions"));
    assert_eq!(*state.get(), VnState::TitleScreen);
    if let Some(pc) = *captured {
        assert_eq!(engine.0.state.pc, pc);
        if now > 3.0 {
            std::fs::write(
                qa.dir.join("navigation.txt"),
                serde_json::to_string(error).unwrap(),
            )
            .unwrap();
            exit.send(AppExit::Success);
        }
    } else {
        *captured = Some(engine.0.state.pc);
    }
}
fn conditional_cards_drive(
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    paths: Res<crate::project_paths::ProjectPaths>,
    mut save: ResMut<crate::systems::save_menu::SaveMenuState>,
    texts: Query<&Text>,
    slots: Query<(&crate::systems::save_menu::SaveSlotButton, &Node)>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize)>,
) {
    use crate::systems::save_menu::{SaveMenuMode, SaveMenuOrigin};
    let now = qa.started.elapsed().as_secs_f64();
    if now > 20.0 {
        panic!("Conditional cards timed out at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.7 {
        return;
    }
    match phase.0 {
        0 => next.set(VnState::Stepping),
        1 if *state.get() == VnState::Waiting => {
            let manager = rvn_core::save::SaveManager::new(&paths.saves, 1000).unwrap();
            engine
                .0
                .save(&manager, 1, "Original".into(), "test.rvn".into())
                .unwrap();
            manager.set_protected(1, true).unwrap();
            phase.2 = engine.0.state.pc;
            save.open(SaveMenuMode::Load, SaveMenuOrigin::InGame);
            next.set(VnState::Menu);
        }
        2 => {
            let count = |value: &str| {
                texts
                    .iter()
                    .flat_map(|t| &t.sections)
                    .filter(|s| s.value == value)
                    .count()
            };
            assert_eq!(count("EMPLACEMENT VIDE"), 5);
            assert_eq!(count("SAUVEGARDE PRESENTE"), 1);
            assert_eq!(count("PROTEGEE"), 1);
            assert_eq!(count("NON PROTEGEE"), 5);
            assert_eq!(count("FILTRE EMPLACEMENT VIDE"), 0);
            assert_eq!(count("FILTRE SAUVEGARDE PRESENTE"), 1);
            assert_eq!(count("FILTRE PROTEGEE"), 1);
            assert_eq!(count("FILTRE NON PROTEGEE"), 0);
            let slots: Vec<_> = slots
                .iter()
                .filter(|(_, node)| node.size().x > 0.0 && node.size().y > 0.0)
                .collect();
            assert_eq!(
                slots.len(),
                2,
                "Hidden or empty load cards retained an active target"
            );
            assert!(slots.iter().all(|(s, _)| s.0 == 1));
            assert_eq!(engine.0.state.pc, phase.2);
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("conditional-cards.png"))
                .unwrap();
            std::fs::write(qa.dir.join("cards.txt"),"Empty, filled and protected decorations are data-bound; conditional roots have no hidden activation target; story unchanged.").unwrap();
        }
        3 => {
            exit.send(AppExit::Success);
        }
        _ => return,
    }
    phase.0 += 1;
    phase.1 = now;
}
fn disabled_control_drive(
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    menus: Res<crate::menu_documents::Menus>,
    buttons: Query<(&GlobalTransform, &Interaction), With<crate::menu_documents::UiInputBlocker>>,
    mut pointer: ResMut<QaPointer>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 20.0 {
        panic!("Disabled control timeout at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.6 {
        return;
    }
    match phase.0 {
        0 => next.set(VnState::Stepping),
        1 if *state.get() == VnState::Waiting => {
            let Some((t, _)) = buttons.iter().next() else {
                return;
            };
            phase.2 = engine.0.state.pc;
            pointer.position = Some(t.translation().truncate());
        }
        2 => {
            assert!(buttons.iter().any(|(_, i)| *i == Interaction::Hovered));
            pointer.buttons.push(bevy::input::ButtonState::Pressed);
        }
        3 => pointer.buttons.push(bevy::input::ButtonState::Released),
        4 => {
            assert_eq!(
                engine.0.state.pc, phase.2,
                "Click passed through the disabled control"
            );
            assert!(menus.session.variables.get("quick_activated").is_none());
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("disabled-control.png"))
                .unwrap();
            std::fs::write(qa.dir.join("disabled-control.txt"),"The disabled quick-action absorbs pointer activation, without firing its graph or advancing the dialogue.").unwrap();
        }
        5 => {
            exit.send(AppExit::Success);
        }
        _ => return,
    }
    phase.0 += 1;
    phase.1 = now;
}
fn disabled_choices_drive(
    qa: Res<Qa>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    mut menus: ResMut<crate::menu_documents::Menus>,
    buttons: Query<(&crate::components::ChoiceButton, &GlobalTransform)>,
    mut pointer: ResMut<QaPointer>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 20.0 {
        panic!("Disabled choices timeout at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.6 {
        return;
    }
    let key = ("choices_0".into(), "responses".into());
    match phase.0 {
        0 => next.set(VnState::Stepping),
        1 if matches!(
            engine.0.current_interaction(),
            Ok(Some(rvn_core::Interaction::Choice { .. }))
        ) =>
        {
            phase.2 = engine.0.state.pc;
            menus.session.presentation.insert(
                key,
                rvn_ui::ElementState {
                    enabled: Some(false),
                    ..default()
                },
            );
        }
        2 => {
            assert!(!menus.choices_interactive());
            let Some((_, t)) = buttons.iter().find(|(b, _)| b.0 == 0) else {
                return;
            };
            pointer.position = Some(t.translation().truncate());
        }
        3 => {
            pointer.buttons.push(bevy::input::ButtonState::Pressed);
            pointer.keys.push((KeyCode::Digit1, true));
        }
        4 => {
            pointer.buttons.push(bevy::input::ButtonState::Released);
            pointer.keys.push((KeyCode::Digit1, false));
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("disabled-choices.png"))
                .unwrap();
        }
        5 => {
            assert_eq!(engine.0.state.pc, phase.2);
            menus.session.presentation.insert(
                key,
                rvn_ui::ElementState {
                    visible: Some(false),
                    ..default()
                },
            );
        }
        6 => {
            assert!(!menus.choices_interactive());
            pointer.keys.push((KeyCode::Enter, true));
        }
        7 => pointer.keys.push((KeyCode::Enter, false)),
        8 => {
            assert_eq!(engine.0.state.pc, phase.2);
            menus.session.presentation.remove(&key);
        }
        9 => {
            assert!(menus.choices_interactive());
            pointer.keys.push((KeyCode::Digit3, true));
        }
        10 => pointer.keys.push((KeyCode::Digit3, false)),
        11 => {
            assert!(
                matches!(engine.0.current_interaction(),Ok(Some(rvn_core::Interaction::Dialogue{text,..})) if text=="Destination 2 atteinte.")
            );
            std::fs::write(qa.dir.join("disabled-choices.txt"),"Disabled choices ignore mouse and number keys; hidden choices ignore Enter. Restoring the interface keeps the original third destination.").unwrap();
        }
        12 => {
            exit.send(AppExit::Success);
        }
        _ => return,
    }
    phase.0 += 1;
    phase.1 = now;
}
fn controller_choices_drive(
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    focus: Res<crate::resources::ChoiceFocus>,
    lists: Query<(&crate::menu_documents::MenuScroll, &Node)>,
    mut pointer: ResMut<QaPointer>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 20.0 {
        panic!("Controller choices timeout at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.6 {
        return;
    }
    match phase.0 {
        0 => next.set(VnState::Stepping),
        1 if matches!(
            engine.0.current_interaction(),
            Ok(Some(rvn_core::Interaction::Choice { .. }))
        ) =>
        {
            phase.2 = engine.0.state.pc;
            pointer.keys.push((KeyCode::ArrowUp, true));
        }
        2 => pointer.keys.push((KeyCode::ArrowUp, false)),
        3 => {
            assert_eq!(focus.0, Some(11));
            assert_eq!(engine.0.state.pc, phase.2);
            let (scroll, node) = lists
                .iter()
                .find(|(s, n)| s.content_height > n.size().y + 20.0)
                .expect("Long choices must scroll");
            assert!(scroll.offset > 0.0);
            assert!(
                (scroll.offset - (scroll.content_height - node.size().y)).abs() < 2.0,
                "Last response is clipped"
            );
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("controller-choices.png"))
                .unwrap();
            pointer.keys.push((KeyCode::Enter, true));
        }
        4 => pointer.keys.push((KeyCode::Enter, false)),
        5 if *state.get() == VnState::Waiting => {
            assert!(
                matches!(engine.0.current_interaction(),Ok(Some(rvn_core::Interaction::Dialogue{text,..})) if text=="Destination 11 atteinte.")
            );
            std::fs::write(qa.dir.join("controller-choices.txt"),"Controller selects and reveals the twelfth response; its original destination is preserved, with no double activation.").unwrap();
        }
        6 => {
            exit.send(AppExit::Success);
        }
        _ => return,
    }
    phase.0 += 1;
    phase.1 = now;
}
fn controller_focus_drive(
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    menus: Res<crate::menu_documents::Menus>,
    mut pointer: ResMut<QaPointer>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 20.0 {
        panic!("Controller focus timeout at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.6 {
        return;
    }
    match phase.0 {
        0 => next.set(VnState::Stepping),
        1 if *state.get() == VnState::Waiting => {
            phase.2 = engine.0.state.pc;
            pointer.keys.push((KeyCode::ArrowDown, true));
        }
        2 => pointer.keys.push((KeyCode::ArrowDown, false)),
        3 => {
            assert_eq!(
                menus.session.variables.get("quick_focused"),
                Some(&true.into())
            );
            pointer.keys.push((KeyCode::Enter, true));
        }
        4 => pointer.keys.push((KeyCode::Enter, false)),
        5 => {
            assert_eq!(
                menus.session.variables.get("quick_activated"),
                Some(&true.into())
            );
            assert_eq!(engine.0.state.pc, phase.2);
            assert_eq!(*state.get(), VnState::Waiting);
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("controller-focus.png"))
                .unwrap();
            std::fs::write(qa.dir.join("controller.txt"),"Controller initializes quick-action focus and activates its graph without advancing the story.").unwrap();
        }
        6 => {
            exit.send(AppExit::Success);
        }
        _ => return,
    }
    phase.0 += 1;
    phase.1 = now;
}
fn long_dialogue_drive(
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    mut typing: ResMut<crate::resources::TypewriterState>,
    lists: Query<(&crate::menu_documents::MenuScroll, &Node)>,
    mut pointer: ResMut<QaPointer>,
    mut input: EventWriter<PlayerInput>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 25.0 {
        panic!("Long dialogue QA timed out at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.6 {
        return;
    }
    match phase.0 {
        0 => {
            next.set(VnState::Stepping);
        }
        1 if *state.get() == VnState::Waiting => {
            phase.2 = engine.0.state.pc;
            typing.skip();
        }
        2 => {
            let Some((scroll, node)) = lists
                .iter()
                .find(|(s, n)| s.content_height > n.size().y + 20.0)
            else {
                return;
            };
            assert!(scroll.offset > 0.0);
            assert!((scroll.offset - (scroll.content_height - node.size().y)).abs() < 1.0);
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("dialogue-end.png"))
                .unwrap();
            pointer.keys.push((KeyCode::PageUp, true));
        }
        3 => {
            pointer.keys.push((KeyCode::PageUp, false));
        }
        4 => {
            let Some((scroll, node)) = lists
                .iter()
                .find(|(s, n)| s.content_height > n.size().y + 20.0)
            else {
                return;
            };
            assert!(scroll.offset < scroll.content_height - node.size().y - 20.0);
            assert_eq!(engine.0.state.pc, phase.2);
            assert_eq!(*state.get(), VnState::Waiting);
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("dialogue-read-back.png"))
                .unwrap();
            input.send(PlayerInput::Advance);
        }
        5 if *state.get() == VnState::Waiting && engine.0.state.pc != phase.2 => {
            typing.skip();
        }
        6 => {
            assert!(lists
                .iter()
                .all(|(s, n)| s.offset == 0.0 && s.content_height <= n.size().y));
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("dialogue-short.png"))
                .unwrap();
            std::fs::write(qa.dir.join("dialogue.txt"),"Twenty paragraphs remain scrollable; PageUp does not advance history; next short dialogue resets offset.").unwrap();
        }
        7 => {
            exit.send(AppExit::Success);
        }
        _ => return,
    }
    phase.0 += 1;
    phase.1 = now;
}
fn long_dropdown_drive(
    qa: Res<Qa>,
    menus: Res<crate::menu_documents::Menus>,
    engine: Res<VnEngine>,
    mut settings_menu: ResMut<crate::systems::settings_menu::SettingsMenuState>,
    mut settings: ResMut<crate::systems::settings_menu::Settings>,
    mut locales: ResMut<crate::resources::LocaleConfig>,
    dropdown: Res<crate::menu_documents::Dropdown>,
    selectors: Query<(&GlobalTransform, &Interaction), With<crate::menu_documents::LanguageSelect>>,
    lists: Query<(&crate::menu_documents::MenuScroll, &Node)>,
    mut pointer: ResMut<QaPointer>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 25.0 {
        panic!("Long dropdown QA timed out at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.6 {
        return;
    }
    match phase.0 {
        0 => {
            phase.2 = engine.0.state.pc;
            settings_menu.active = true;
            locales.available_langs = (0..30).map(|i| format!("lang_{i:02}")).collect();
            settings.language = "lang_29".into();
        }
        1 => {
            let Ok((t, _)) = selectors.get_single() else {
                return;
            };
            pointer.position = Some(t.translation().truncate());
        }
        2 => {
            if !selectors.iter().any(|(_, i)| *i == Interaction::Hovered) {
                return;
            }
            pointer.buttons.push(bevy::input::ButtonState::Pressed);
        }
        3 => {
            pointer.buttons.push(bevy::input::ButtonState::Released);
        }
        4 => {
            if !dropdown.open {
                return;
            }
            let Some((scroll, node)) = lists.iter().find(|(s, _)| s.content_height == 1260.0)
            else {
                return;
            };
            assert!(scroll.offset > 0.0);
            assert!(
                (scroll.offset + node.size().y - 1260.0).abs() < 2.0,
                "Selected last option is clipped"
            );
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("dropdown-last.png"))
                .unwrap();
            pointer.keys.push((KeyCode::ArrowDown, true));
        }
        5 => {
            pointer.keys.push((KeyCode::ArrowDown, false));
        }
        6 => {
            let Some((scroll, _)) = lists.iter().find(|(s, _)| s.content_height == 1260.0) else {
                return;
            };
            assert_eq!(
                scroll.offset, 0.0,
                "Wrapped keyboard focus was not revealed"
            );
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("dropdown-first.png"))
                .unwrap();
            pointer.keys.push((KeyCode::Enter, true));
        }
        7 => {
            pointer.keys.push((KeyCode::Enter, false));
        }
        8 => {
            assert!(!dropdown.open);
            assert_eq!(settings.language, "lang_00");
            assert_eq!(
                menus.session.variables.get("language_changed"),
                Some(&true.into()),
                "Inherited setting did not dispatch ValueChanged"
            );
            assert_eq!(engine.0.state.pc, phase.2);
            std::fs::write(qa.dir.join("dropdown.txt"),"Thirty languages: selected last row revealed on open, keyboard wrap scrolls to first, selection confirmed, story unchanged.").unwrap();
        }
        _ => {
            exit.send(AppExit::Success);
            return;
        }
    }
    phase.0 += 1;
    phase.1 = now;
}
fn text_states_drive(
    qa: Res<Qa>,
    engine: Res<VnEngine>,
    nodes: Query<(
        &crate::menu_documents::VisualNode,
        &GlobalTransform,
        &Interaction,
    )>,
    texts: Query<&Text>,
    image_nodes: Query<(&Parent, &UiImage, &Visibility)>,
    visuals: Query<&crate::menu_documents::VisualNode>,
    assets: Res<AssetServer>,
    mut pointer: ResMut<QaPointer>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 20.0 {
        panic!("Text state QA timed out at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.6 {
        return;
    }
    let paint = |label: &str| {
        texts
            .iter()
            .flat_map(|t| &t.sections)
            .find(|s| s.value == label)
            .map(|s| s.style.color)
    };
    if std::env::var_os("RVN_QA_IMAGE_STATES").is_some() {
        let image = |id: &str| {
            image_nodes
                .iter()
                .find(|(parent, _, _)| visuals.get(parent.get()).is_ok_and(|v| v.id == id))
                .map(|(_, image, visibility)| {
                    (
                        assets
                            .get_path(image.texture.id())
                            .map(|p| p.path().to_string_lossy().to_string()),
                        *visibility,
                    )
                })
        };
        match phase.0 {
            0 => {
                assert_eq!(
                    image("disabled").unwrap().0.as_deref(),
                    Some("disabled.png")
                );
                assert_eq!(
                    image("selected").unwrap().0.as_deref(),
                    Some("selected.png")
                );
            }
            1 => assert_eq!(
                image("interactive").unwrap().0.as_deref(),
                Some("hover.png")
            ),
            3 => assert_eq!(image("interactive").unwrap().1, Visibility::Hidden),
            6 => assert!(["interactive", "selected"].iter().any(|id| image(id)
                .is_some_and(|(path, visible)| path.as_deref() == Some("focus.png")
                    && visible == Visibility::Inherited))),
            _ => {}
        }
    }
    match phase.0 {
        0 => {
            phase.2 = engine.0.state.pc;
            assert_eq!(
                paint("Désactivé"),
                Some(Color::srgba(0.25, 0.25, 0.25, 1.0))
            );
            assert_eq!(paint("Sélectionné"), Some(Color::srgba(0.0, 1.0, 0.0, 1.0)));
            let Some((_, t, _)) = nodes.iter().find(|(n, _, _)| n.id == "interactive") else {
                return;
            };
            pointer.position = Some(t.translation().truncate());
        }
        1 => {
            assert_eq!(
                paint("Survol et clic"),
                Some(Color::srgba(1.0, 1.0, 1.0, 1.0))
            );
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("text-hover.png"))
                .unwrap();
        }
        2 => {
            pointer.buttons.push(bevy::input::ButtonState::Pressed);
        }
        3 => {
            assert_eq!(
                paint("Survol et clic"),
                Some(Color::srgba(1.0, 0.0, 0.0, 1.0))
            );
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("text-pressed.png"))
                .unwrap();
        }
        4 => {
            pointer.buttons.push(bevy::input::ButtonState::Released);
            pointer.position = Some(Vec2::new(5.0, 5.0));
            pointer.keys.push((KeyCode::ArrowDown, true));
        }
        5 => {
            pointer.keys.push((KeyCode::ArrowDown, false));
        }
        6 => {
            assert!(
                paint("Survol et clic") == Some(Color::srgba(0.0, 0.5, 1.0, 1.0))
                    || paint("Sélectionné") == Some(Color::srgba(0.0, 0.5, 1.0, 1.0))
            );
            assert_eq!(engine.0.state.pc, phase.2);
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("text-focus.png"))
                .unwrap();
            std::fs::write(qa.dir.join("text-states.txt"),"Hover, pressed, disabled, selected and keyboard-focus foreground colors rendered independently; narrative unchanged.").unwrap();
        }
        _ => {
            exit.send(AppExit::Success);
            return;
        }
    }
    phase.0 += 1;
    phase.1 = now;
}
fn resume_thumbnail_drive(
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    paths: Res<crate::project_paths::ProjectPaths>,
    mut typing: ResMut<crate::resources::TypewriterState>,
    mut input: EventWriter<PlayerInput>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize, String)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 30.0 {
        panic!("Resume thumbnail QA timed out at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.5 {
        return;
    }
    let manager = rvn_core::save::SaveManager::new(&paths.saves, 1000).unwrap();
    let image = |data: &rvn_core::save::SaveData| {
        data.thumbnail
            .as_ref()
            .and_then(|p| p.strip_prefix("@save/"))
            .map(|p| paths.saves.join(p))
            .filter(|p| p.is_file())
    };
    match phase.0 {
        0 => next.set(VnState::Stepping),
        1 => {
            if *state.get() != VnState::Waiting {
                return;
            }
            phase.2 = engine.0.state.pc;
            typing.skip();
        }
        2 => {
            let data = manager.load_autosave().unwrap();
            let Some(path) = image(&data) else {
                return;
            };
            assert_eq!(data.pc, phase.2);
            assert_eq!(engine.0.state.pc, phase.2);
            phase.3 = data.thumbnail.clone().unwrap();
            std::fs::copy(path, qa.dir.join("autosave-first.png")).unwrap();
            input.send(PlayerInput::QuickSave);
        }
        3 => {
            let Ok(data) = manager.load_quicksave() else {
                return;
            };
            let Some(path) = image(&data) else {
                return;
            };
            assert_eq!(data.pc, phase.2);
            std::fs::copy(path, qa.dir.join("quicksave.png")).unwrap();
            assert_eq!(
                std::fs::read(qa.dir.join("autosave-first.png")).unwrap(),
                std::fs::read(qa.dir.join("quicksave.png")).unwrap()
            );
            input.send(PlayerInput::Advance);
        }
        4 => {
            if *state.get() != VnState::Waiting || engine.0.state.pc == phase.2 {
                return;
            }
            typing.skip();
        }
        5 => {
            let data = manager.load_autosave().unwrap();
            let Some(path) = image(&data) else {
                return;
            };
            if data.thumbnail.as_deref() == Some(&phase.3) {
                return;
            }
            assert_eq!(data.pc, engine.0.state.pc);
            assert_eq!(manager.load_quicksave().unwrap().pc, phase.2);
            std::fs::copy(path, qa.dir.join("autosave-second.png")).unwrap();
            assert_ne!(
                std::fs::read(qa.dir.join("autosave-first.png")).unwrap(),
                std::fs::read(qa.dir.join("autosave-second.png")).unwrap()
            );
            std::fs::write(qa.dir.join("resume-thumbnails.txt"),"Automatic and quick saves received matching gameplay screenshots; subsequent scene changed only the automatic save; narrative positions preserved.").unwrap();
        }
        _ => {
            exit.send(AppExit::Success);
            return;
        }
    }
    phase.0 += 1;
    phase.1 = now;
}
fn pointer_drive(
    qa: Res<Qa>,
    engine: Res<VnEngine>,
    settings: Res<crate::systems::settings_menu::SettingsMenuState>,
    nodes: Query<(
        &crate::menu_documents::VisualNode,
        &GlobalTransform,
        Option<&Interaction>,
        Has<Button>,
    )>,
    texts: Query<&Text>,
    mut pointer: ResMut<QaPointer>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, usize)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 20.0 {
        panic!("Pointer graph QA timed out at {}", phase.0);
    }
    if now < 1.0 || now - phase.1 < 0.6 {
        return;
    }
    let label = |value: &str| {
        texts
            .iter()
            .any(|t| t.sections.iter().any(|s| s.value == value))
    };
    match phase.0 {
        0 => {
            phase.2 = engine.0.state.pc;
            let Some((_, t, interaction, button)) =
                nodes.iter().find(|(n, _, _, _)| n.id == "hover_region")
            else {
                return;
            };
            assert!(interaction.is_some());
            assert!(!button, "Hover-only panel gained keyboard focus");
            pointer.position = Some(t.translation().truncate());
        }
        1 => {
            if !label("Survol") {
                return;
            }
            pointer.position = Some(Vec2::new(5.0, 5.0));
        }
        2 => {
            if !label("Sortie") {
                return;
            }
            let Some((_, t, _, button)) = nodes.iter().find(|(n, _, _, _)| n.id == "click_image")
            else {
                return;
            };
            assert!(button);
            pointer.position = Some(t.translation().truncate());
        }
        3 => {
            if !nodes
                .iter()
                .any(|(n, _, i, _)| n.id == "click_image" && i == Some(&Interaction::Hovered))
            {
                return;
            }
            pointer.buttons.push(bevy::input::ButtonState::Pressed);
        }
        4 => {
            pointer.buttons.push(bevy::input::ButtonState::Released);
        }
        5 => {
            if !label("Clic image") {
                return;
            }
            assert_eq!(engine.0.state.pc, phase.2);
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("pointer-events.png"))
                .unwrap();
        }
        6 => {
            let Some((_, t, interaction, button)) =
                nodes.iter().find(|(n, _, _, _)| n.id == "text_shortcut")
            else {
                return;
            };
            assert!(interaction.is_some() && button);
            pointer.position = Some(t.translation().truncate());
        }
        7 => {
            pointer.buttons.push(bevy::input::ButtonState::Pressed);
        }
        8 => {
            pointer.buttons.push(bevy::input::ButtonState::Released);
        }
        9 => {
            assert!(settings.active);
            assert_eq!(engine.0.state.pc, phase.2);
            std::fs::write(qa.dir.join("pointer.txt"),"Panel hover/leave, image graph click and text shortcut dispatched; hover-only panel excluded from focus; narrative unchanged.").unwrap();
        }
        _ => {
            exit.send(AppExit::Success);
            return;
        }
    }
    phase.0 += 1;
    phase.1 = now;
}
fn approve_quickload(
    confirm: Res<crate::systems::save_menu::SaveConfirmation>,
    mut buttons: Query<(&crate::menu_documents::ConfirmationButton, &mut Interaction)>,
) {
    if confirm.pending == Some((0, crate::systems::save_menu::SaveMenuMode::Load)) {
        for (button, mut interaction) in &mut buttons {
            if button.0 {
                *interaction = Interaction::Pressed;
            }
        }
    }
}
fn slots_drive(
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    paths: Res<crate::project_paths::ProjectPaths>,
    confirm: Res<crate::systems::save_menu::SaveConfirmation>,
    mut save: ResMut<crate::systems::save_menu::SaveMenuState>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    slots: Query<(
        &crate::systems::save_menu::SaveSlotButton,
        &crate::systems::save_menu::SaveSlotMode,
        &Interaction,
        &GlobalTransform,
    )>,
    buttons: Query<(
        &crate::menu_documents::ConfirmationButton,
        &Interaction,
        &GlobalTransform,
    )>,
    mut mouse: EventWriter<bevy::input::mouse::MouseButtonInput>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, bool, usize)>,
) {
    use crate::systems::save_menu::{SaveMenuMode as Mode, SaveMenuOrigin};
    let now = qa.started.elapsed().as_secs_f64();
    let (window, mut view) = windows.single_mut();
    if phase.2 && now - phase.1 > 0.15 {
        mouse.send(bevy::input::mouse::MouseButtonInput {
            button: MouseButton::Left,
            state: bevy::input::ButtonState::Released,
            window,
        });
        phase.2 = false;
    }
    if now > 40.0 {
        panic!("Save card QA timed out at phase {}", phase.0);
    }
    let mgr = rvn_core::save::SaveManager::new(&paths.saves, 1000).unwrap();
    if now - phase.1 < 0.5 {
        return;
    }
    match phase.0 {
        0 if now > 1.0 => {
            next.set(VnState::Stepping);
            phase.0 = 1;
        }
        1 if *state.get() == VnState::Waiting => {
            engine
                .0
                .save(&mgr, 1, "Original".into(), "test.rvn".into())
                .unwrap();
            phase.3 = engine.0.state.pc;
            save.open(Mode::Save, SaveMenuOrigin::InGame);
            next.set(VnState::Menu);
            phase.0 = 2;
            phase.1 = now;
        }
        2 | 4 | 6 | 9 => {
            let mode = if matches!(phase.0, 2 | 4) {
                Mode::ToggleProtection
            } else {
                Mode::Delete
            };
            if let Some((_, _, interaction, t)) = slots
                .iter()
                .find(|(slot, m, _, _)| slot.0 == 1 && m.0 == mode)
            {
                view.set_cursor_position(Some(t.translation().truncate()));
                if *interaction == Interaction::Hovered {
                    mouse.send(bevy::input::mouse::MouseButtonInput {
                        button: MouseButton::Left,
                        state: bevy::input::ButtonState::Pressed,
                        window,
                    });
                    phase.0 += 1;
                    phase.1 = now;
                    phase.2 = true;
                }
            }
        }
        3 if mgr.is_protected(1).unwrap() => {
            assert!(!slots
                .iter()
                .any(|(slot, m, _, _)| slot.0 == 1 && matches!(m.0, Mode::Save | Mode::Delete)));
            assert!(slots
                .iter()
                .any(|(slot, m, _, _)| slot.0 == 1 && m.0 == Mode::Load));
            shots
                .save_screenshot_to_disk(window, qa.dir.join("save-protected.png"))
                .unwrap();
            phase.0 = 4;
            phase.1 = now;
        }
        5 if !mgr.is_protected(1).unwrap() => {
            phase.0 = 6;
            phase.1 = now;
        }
        7 | 10 if confirm.pending == Some((1, Mode::Delete)) => {
            let yes = phase.0 == 10;
            if let Some((_, i, t)) = buttons.iter().find(|(b, _, _)| b.0 == yes) {
                view.set_cursor_position(Some(t.translation().truncate()));
                if *i == Interaction::Hovered {
                    mouse.send(bevy::input::mouse::MouseButtonInput {
                        button: MouseButton::Left,
                        state: bevy::input::ButtonState::Pressed,
                        window,
                    });
                    phase.0 += 1;
                    phase.1 = now;
                    phase.2 = true;
                }
            }
        }
        8 if confirm.pending.is_none() => {
            assert_eq!(mgr.load(1).unwrap().label, "Original");
            phase.0 = 9;
            phase.1 = now;
        }
        11 if confirm.pending.is_none() && !mgr.slot_occupied(1) => {
            assert_eq!(engine.0.state.pc, phase.3);
            assert!(save.active);
            shots
                .save_screenshot_to_disk(window, qa.dir.join("save-deleted.png"))
                .unwrap();
            std::fs::write(qa.dir.join("slots.txt"),"Protected cards disable Save/Delete, preserve Load; unlock, delete cancellation and approval preserve the story and origin page.").unwrap();
            phase.0 = 12;
            phase.1 = now;
        }
        12 => {
            exit.send(AppExit::Success);
        }
        _ => {}
    }
}
fn animation_drive(
    qa: Res<Qa>,
    engine: Res<VnEngine>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    nodes: Query<(
        &crate::menu_documents::VisualNode,
        &GlobalTransform,
        &BackgroundColor,
    )>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, usize)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    let (window, view) = windows.single();
    let Some((_, transform, color)) = nodes.iter().find(|(n, _, _)| n.id == "animated") else {
        if now > 10.0 {
            panic!("Animation target absent");
        }
        return;
    };
    match phase.0 {
        0 if now > 0.6 => {
            phase.1 = engine.0.state.pc;
            shots
                .save_screenshot_to_disk(window, qa.dir.join("animation-start.png"))
                .unwrap();
            phase.0 = 1;
        }
        1 if now > 4.5 => {
            let scale = (view.width() / 1920.0).min(view.height() / 1080.0);
            assert!(
                (transform.translation().x - 950.0 * scale).abs() < 2.0,
                "Movement accumulated instead of finishing at its target"
            );
            assert!((transform.compute_transform().scale.x - 1.0).abs() < 0.01);
            let c = color.0.to_srgba();
            assert!(
                (c.alpha - 1.0).abs() < 0.01 && (c.blue - 0.5).abs() < 0.01,
                "Final effect color: {c:?}"
            );
            assert_eq!(engine.0.state.pc, phase.1);
            shots
                .save_screenshot_to_disk(window, qa.dir.join("animation-end.png"))
                .unwrap();
            std::fs::write(
                qa.dir.join("animation.txt"),
                "Four simultaneous effects reached their endpoints without advancing the story",
            )
            .unwrap();
            phase.0 = 2;
        }
        2 if now > 6.0 => {
            exit.send(AppExit::Success);
        }
        _ => {}
    }
}
fn dropdown_drive(
    qa: Res<Qa>,
    engine: Res<VnEngine>,
    mut settings_menu: ResMut<crate::systems::settings_menu::SettingsMenuState>,
    settings: Res<crate::systems::settings_menu::Settings>,
    mut locales: ResMut<crate::resources::LocaleConfig>,
    dropdown: Res<crate::menu_documents::Dropdown>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    selectors: Query<(&GlobalTransform, &Interaction), With<crate::menu_documents::LanguageSelect>>,
    mut mouse: EventWriter<bevy::input::mouse::MouseButtonInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, Option<usize>)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 25.0 {
        panic!("Dropdown test timed out");
    }
    if now < 1.5 || now - phase.1 < 0.7 {
        return;
    }
    let (window, mut view) = windows.single_mut();
    match phase.0 {
        0 => {
            phase.2 = Some(engine.0.state.pc);
            settings_menu.active = true;
            locales.available_langs = vec!["en".into(), "fr".into()];
        }
        1 | 8 => {
            let Ok((transform, _)) = selectors.get_single() else {
                return;
            };
            view.set_cursor_position(Some(transform.translation().truncate()));
        }
        2 | 9 => {
            if !selectors.iter().any(|(_, i)| *i == Interaction::Hovered) {
                return;
            }
            mouse.send(bevy::input::mouse::MouseButtonInput {
                button: MouseButton::Left,
                state: bevy::input::ButtonState::Pressed,
                window,
            });
        }
        3 | 10 => {
            mouse.send(bevy::input::mouse::MouseButtonInput {
                button: MouseButton::Left,
                state: bevy::input::ButtonState::Released,
                window,
            });
            assert!(dropdown.open);
            shots
                .save_screenshot_to_disk(window, qa.dir.join(format!("dropdown-{}.png", phase.0)))
                .unwrap();
        }
        4 => {
            keys.press(KeyCode::ArrowUp);
        }
        5 => {
            keys.release(KeyCode::ArrowUp);
            keys.press(KeyCode::Enter);
        }
        6 => {
            keys.release(KeyCode::Enter);
            assert!(!dropdown.open);
            assert_eq!(settings.language, "en");
        }
        7 => {
            assert_eq!(phase.2, Some(engine.0.state.pc));
            shots
                .save_screenshot_to_disk(window, qa.dir.join("dropdown-selected.png"))
                .unwrap();
        }
        11 => {
            view.set_cursor_position(Some(Vec2::new(5.0, 5.0)));
        }
        12 => {
            mouse.send(bevy::input::mouse::MouseButtonInput {
                button: MouseButton::Left,
                state: bevy::input::ButtonState::Pressed,
                window,
            });
        }
        13 => {
            mouse.send(bevy::input::mouse::MouseButtonInput {
                button: MouseButton::Left,
                state: bevy::input::ButtonState::Released,
                window,
            });
            assert!(!dropdown.open);
            assert_eq!(settings.language, "en");
            assert_eq!(phase.2, Some(engine.0.state.pc));
            std::fs::write(qa.dir.join("dropdown.txt"),"Language popup: keyboard selection, outside cancellation and unchanged story verified").unwrap();
        }
        _ => {
            exit.send(AppExit::Success);
            return;
        }
    }
    phase.0 += 1;
    phase.1 = now;
}
fn scroll_drive(
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    lists: Query<(&crate::menu_documents::MenuScroll, &Node, &GlobalTransform)>,
    mut wheel: EventWriter<bevy::input::mouse::MouseWheel>,
    mut pointer: ResMut<QaPointer>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, Option<usize>)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if std::env::var_os("RVN_QA_SCROLL_GAMEPLAY").is_some()
        && *state.get() == VnState::TitleScreen
        && phase.2.is_none()
        && now > 1.0
    {
        phase.2 = Some(engine.0.state.pc);
        next.set(VnState::Stepping);
        return;
    }
    if now > 20.0 {
        panic!(
            "Nested scrolling test timed out at phase {}: state {:?}, pc {}, lists {}",
            phase.0,
            state.get(),
            engine.0.state.pc,
            lists.iter().count()
        );
    }
    if now < 1.5 || now - phase.1 < 0.7 {
        return;
    }
    let mut lists: Vec<_> = lists
        .iter()
        .filter(|(scroll, node, _)| scroll.content_height > node.size().y + 0.5)
        .collect();
    if lists.len() != 2 {
        return;
    }
    lists.sort_by(|a, b| a.1.size().y.total_cmp(&b.1.size().y));
    let inner = lists[0];
    let outer = lists[1];
    let (window, mut view) = windows.single_mut();
    match phase.0 {
        0 => {
            phase.2 = Some(engine.0.state.pc);
            shots
                .save_screenshot_to_disk(window, qa.dir.join("scroll-before.png"))
                .unwrap();
        }
        1 => {
            view.set_cursor_position(Some(inner.2.translation().truncate()));
            wheel.send(bevy::input::mouse::MouseWheel {
                unit: bevy::input::mouse::MouseScrollUnit::Line,
                x: 0.0,
                y: -4.0,
                window,
            });
        }
        2 => {
            assert!(inner.0.offset > 0.0);
            assert_eq!(
                outer.0.offset, 0.0,
                "Both nested zones consumed the same wheel event"
            );
            shots
                .save_screenshot_to_disk(window, qa.dir.join("scroll-inner.png"))
                .unwrap();
        }
        3 => {
            view.set_cursor_position(Some(
                outer.2.translation().truncate() - Vec2::new(outer.1.size().x * 0.35, 0.0),
            ));
            wheel.send(bevy::input::mouse::MouseWheel {
                unit: bevy::input::mouse::MouseScrollUnit::Line,
                x: 0.0,
                y: -4.0,
                window,
            });
        }
        4 => {
            assert!(outer.0.offset > 0.0);
            assert_eq!(phase.2, Some(engine.0.state.pc));
            shots
                .save_screenshot_to_disk(window, qa.dir.join("scroll-outer.png"))
                .unwrap();
            std::fs::write(
                qa.dir.join("scroll.txt"),
                "Nested wheel routing and unchanged narrative position verified",
            )
            .unwrap();
        }
        5 => {
            view.set_cursor_position(Some(
                outer.2.translation().truncate()
                    + Vec2::new(outer.1.size().x * 0.5 - 6.0, -outer.1.size().y * 0.25),
            ));
        }
        6 => {
            pointer.buttons.push(bevy::input::ButtonState::Pressed);
        }
        7 => {
            view.set_cursor_position(Some(
                outer.2.translation().truncate()
                    + Vec2::new(outer.1.size().x * 0.5 - 6.0, outer.1.size().y * 0.5 - 3.0),
            ));
        }
        8 => {
            pointer.buttons.push(bevy::input::ButtonState::Released);
        }
        9 => {
            assert!(
                outer.0.offset >= (outer.0.content_height - outer.1.size().y) * 0.95,
                "Dragging the scrollbar did not reach the bottom"
            );
            assert_eq!(phase.2, Some(engine.0.state.pc));
            shots
                .save_screenshot_to_disk(window, qa.dir.join("scroll-drag.png"))
                .unwrap();
            std::fs::write(
                qa.dir.join("scroll-drag.txt"),
                "Scrollbar drag reached bottom without advancing the story",
            )
            .unwrap();
        }
        10 if std::env::var_os("RVN_QA_SCROLL_GAMEPLAY").is_some() => {
            view.set_cursor_position(Some(outer.2.translation().truncate()));
            wheel.send(bevy::input::mouse::MouseWheel {
                unit: bevy::input::mouse::MouseScrollUnit::Line,
                x: 0.0,
                y: 3.0,
                window,
            });
        }
        _ => {
            if std::env::var_os("RVN_QA_SCROLL_GAMEPLAY").is_some() {
                assert_eq!(
                    *state.get(),
                    VnState::Waiting,
                    "UI scrolling opened narrative history"
                );
                assert_eq!(phase.2, Some(engine.0.state.pc));
            }
            exit.send(AppExit::Success);
            return;
        }
    }
    pointer.position = view.cursor_position();
    phase.0 += 1;
    phase.1 = now;
}
#[derive(bevy_ecs::system::SystemParam)]
struct QaCapture<'w> {
    thumbnails: Res<'w, crate::save_thumbnails::SaveThumbnails>,
    typing: ResMut<'w, crate::resources::TypewriterState>,
    pointer: ResMut<'w, QaPointer>,
}
#[derive(Resource, Default)]
struct QaPointer {
    position: Option<Vec2>,
    buttons: Vec<bevy::input::ButtonState>,
    keys: Vec<(KeyCode, bool)>,
}
fn apply_qa_pointer(
    mut pointer: ResMut<QaPointer>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut pads: ResMut<ButtonInput<GamepadButton>>,
) {
    if let Some(position) = pointer.position {
        if let Ok(mut window) = windows.get_single_mut() {
            window.set_cursor_position(Some(position));
        }
    }
    for event in pointer.buttons.drain(..) {
        match event {
            bevy::input::ButtonState::Pressed => mouse.press(MouseButton::Left),
            bevy::input::ButtonState::Released => mouse.release(MouseButton::Left),
        }
    }
    for (key, pressed) in pointer.keys.drain(..) {
        let button = if std::env::var_os("RVN_QA_GAMEPAD").is_some() {
            match key {
                KeyCode::ArrowUp => Some(GamepadButtonType::DPadUp),
                KeyCode::ArrowDown => Some(GamepadButtonType::DPadDown),
                KeyCode::ArrowLeft => Some(GamepadButtonType::DPadLeft),
                KeyCode::ArrowRight => Some(GamepadButtonType::DPadRight),
                KeyCode::Enter => Some(GamepadButtonType::South),
                KeyCode::Escape => Some(GamepadButtonType::East),
                _ => None,
            }
        } else {
            None
        };
        if let Some(button) = button {
            let button = GamepadButton::new(Gamepad::new(0), button);
            if pressed {
                pads.press(button);
            } else {
                pads.release(button);
            }
        } else if pressed {
            keys.press(key);
        } else {
            keys.release(key);
        }
    }
}
fn general_confirm_drive(
    menus: Res<crate::menu_documents::Menus>,
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    paths: Res<crate::project_paths::ProjectPaths>,
    confirm: Res<crate::systems::save_menu::SaveConfirmation>,
    mut persistent: ResMut<crate::resources::PersistentDataResource>,
    windows: Query<Entity, With<PrimaryWindow>>,
    buttons: Query<(
        &crate::menu_documents::ConfirmationButton,
        &Interaction,
        &GlobalTransform,
    )>,
    (controls, visuals): (
        Query<(
            &crate::menu_documents::MenuElement,
            &Interaction,
            &GlobalTransform,
        )>,
        Query<(&crate::menu_documents::VisualNode, &GlobalTransform)>,
    ),
    mut pointer: ResMut<QaPointer>,
    mut input: EventWriter<PlayerInput>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, bool, usize, usize)>,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if now > 35.0 {
        panic!(
            "General confirmation QA timed out: phase {} / {:?}",
            phase.0,
            state.get()
        );
    }
    if phase.2 && now - phase.1 > 0.15 {
        pointer.buttons.push(bevy::input::ButtonState::Released);
        phase.2 = false;
    }
    if now - phase.1 < 0.5 {
        return;
    }
    let mgr = rvn_core::save::SaveManager::new(&paths.saves, 1000).unwrap();
    match phase.0 {
        0 if now > 1.0 => {
            next.set(VnState::Stepping);
            phase.0 = 1;
        }
        1 if *state.get() == VnState::Waiting => {
            engine
                .0
                .save(&mgr, 1, "Original".into(), "test.rvn".into())
                .unwrap();
            persistent.data.last_resume_target = Some(
                rvn_core::persistent::LastResumeTarget::manual(1, mgr.load(1).unwrap().timestamp),
            );
            phase.3 = engine.0.state.pc;
            input.send(PlayerInput::Advance);
            phase.0 = 2;
            phase.1 = now;
        }
        2 if *state.get() == VnState::Waiting && engine.0.state.pc != phase.3 => {
            phase.4 = engine.0.state.pc;
            next.set(VnState::Menu);
            phase.0 = 3;
            phase.1 = now;
        }
        3 | 6 | 9 => {
            let action = if phase.0 == 6 {
                rvn_ui::Action::Continue
            } else {
                rvn_ui::Action::NewGame
            };
            if let Some((_, interaction, t)) = controls.iter().find(|(e, _, _)| e.action == action)
            {
                pointer.position = Some(t.translation().truncate());
                if *interaction == Interaction::Hovered {
                    pointer.buttons.push(bevy::input::ButtonState::Pressed);
                    phase.2 = true;
                    phase.0 += 1;
                    phase.1 = now;
                }
            }
        }
        4 if confirm.action_pending.is_some() => {
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("restart-confirmation.png"))
                .unwrap();
            if std::env::var_os("RVN_QA_CONFIRM_GRAPH").is_some() {
                phase.0 = 40;
            } else {
                pointer.keys.push((KeyCode::Escape, true));
                phase.0 = 5;
            }
            phase.1 = now;
        }
        40 => {
            if let Some((_, t)) = visuals.iter().find(|(v, _)| v.id == "confirmation_help") {
                pointer.position = Some(t.translation().truncate());
                phase.0 = 41;
                phase.1 = now;
            }
        }
        41 => {
            pointer.buttons.push(bevy::input::ButtonState::Pressed);
            phase.2 = true;
            phase.0 = 42;
            phase.1 = now;
        }
        42 => {
            for key in ["help_clicked", "help_hovered"] {
                assert_eq!(
                    menus.session.variables.get(key),
                    Some(&true.into()),
                    "Missing decorative modal event: {key}"
                );
            }
            assert!(confirm.active());
            assert_eq!(engine.0.state.pc, phase.4);
            pointer.keys.push((KeyCode::Escape, true));
            phase.0 = 5;
            phase.1 = now;
        }
        5 if !confirm.active() => {
            pointer.keys.push((KeyCode::Escape, false));
            assert_eq!(engine.0.state.pc, phase.4);
            assert_eq!(*state.get(), VnState::Menu);
            phase.0 = 6;
            phase.1 = now;
        }
        7 | 10 if confirm.action_pending.is_some() => {
            if let Some((_, interaction, t)) = buttons.iter().find(|(button, _, _)| button.0) {
                pointer.position = Some(t.translation().truncate());
                if *interaction == Interaction::Hovered {
                    pointer.buttons.push(bevy::input::ButtonState::Pressed);
                    phase.2 = true;
                    phase.0 += 1;
                    phase.1 = now;
                }
            }
        }
        8 if !confirm.active() && *state.get() == VnState::Waiting => {
            if std::env::var_os("RVN_QA_CONFIRM_GRAPH").is_some() {
                for key in [
                    "confirmation_clicked",
                    "confirmation_opened",
                    "confirmation_closed",
                    "confirmation_focused",
                    "confirmation_hovered",
                    "confirmation_left",
                ] {
                    assert_eq!(
                        menus.session.variables.get(key),
                        Some(&serde_json::Value::Bool(true)),
                        "Confirmation event was bypassed: {key}"
                    );
                }
            }
            assert_eq!(engine.0.state.pc, phase.3);
            next.set(VnState::Menu);
            phase.0 = 9;
            phase.1 = now;
        }
        11 if !confirm.active() && *state.get() == VnState::Waiting => {
            assert_eq!(engine.0.state.pc, phase.3);
            assert_eq!(mgr.load(1).unwrap().label, "Original");
            std::fs::write(qa.dir.join("general-confirmations.txt"),"Restart cancellation preserves the active story and page. Continue and restart approvals execute once. Fallback confirmation works without a custom Confirm page.").unwrap();
            exit.send(AppExit::Success);
            phase.0 = 12;
        }
        _ => {}
    }
}
fn confirm_drive(
    qa: Res<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    paths: Res<crate::project_paths::ProjectPaths>,
    mut confirm: ResMut<crate::systems::save_menu::SaveConfirmation>,
    mut save: ResMut<crate::systems::save_menu::SaveMenuState>,
    windows: Query<Entity, With<PrimaryWindow>>,
    buttons: Query<(
        &crate::menu_documents::ConfirmationButton,
        &Interaction,
        &GlobalTransform,
    )>,
    mut shots: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut phase: Local<(usize, f64, bool)>,
    mut input: EventWriter<PlayerInput>,
    mut quick_pc: Local<Option<usize>>,
    mut capture: QaCapture,
) {
    use crate::systems::save_menu::{SaveMenuMode, SaveMenuOrigin};
    let now = qa.started.elapsed().as_secs_f64();
    let window = windows.single();
    if phase.2 && now - phase.1 > 0.15 {
        capture
            .pointer
            .buttons
            .push(bevy::input::ButtonState::Released);
        phase.2 = false;
    }
    if now > 30.0 {
        panic!("Confirmation QA timed out at phase {}", phase.0);
    }
    let mgr = rvn_core::save::SaveManager::new(&paths.saves, 1000).unwrap();
    match phase.0 {
        0 => {
            if now > 1.0 {
                next.set(VnState::Stepping);
                phase.0 = 1;
            }
        }
        1 if *state.get() == VnState::Waiting => {
            if !capture.thumbnails.ready(&engine) {
                capture.typing.skip();
                return;
            }
            engine
                .0
                .save(&mgr, 1, "Before confirmation".into(), "test.rvn".into())
                .unwrap();
            save.open(SaveMenuMode::Save, SaveMenuOrigin::InGame);
            next.set(VnState::Menu);
            confirm.pending = Some((1, SaveMenuMode::Save));
            phase.0 = 2;
            phase.1 = now;
        }
        2 | 4 if now - phase.1 > 1.0 => {
            let approved = phase.0 == 4;
            if let Some((_, i, t)) = buttons.iter().find(|(b, _, _)| b.0 == approved) {
                capture.pointer.position = Some(t.translation().truncate());
                if *i == Interaction::Hovered {
                    shots
                        .save_screenshot_to_disk(
                            window,
                            qa.dir.join(format!("confirmation-{}.png", phase.0)),
                        )
                        .unwrap();
                    capture
                        .pointer
                        .buttons
                        .push(bevy::input::ButtonState::Pressed);
                    phase.2 = true;
                    phase.0 += 1;
                    phase.1 = now;
                }
            }
        }
        3 if confirm.pending.is_none() => {
            assert_eq!(mgr.load(1).unwrap().label, "Before confirmation");
            assert!(save.active);
            confirm.pending = Some((1, SaveMenuMode::Save));
            phase.0 = 4;
            phase.1 = now;
        }
        5 if now - phase.1 > 1.0 => {
            assert_eq!(mgr.load(1).unwrap().label, "Sauvegarde 1");
            assert!(mgr
                .load(1)
                .unwrap()
                .thumbnail
                .unwrap()
                .starts_with("@save/thumbnail_"));
            assert!(!save.active);
            std::fs::write(qa.dir.join("confirmation.txt"),"Cancel preserves the save and origin page; approve replaces only the selected slot.").unwrap();
            save.open(SaveMenuMode::Save, SaveMenuOrigin::InGame);
            phase.0 = 12;
            phase.1 = now;
        }
        11 if *state.get() == VnState::Waiting && now - phase.1 > 0.3 => {
            mgr.save_quicksave(&engine.0.state, "QA quicksave".into(), "test.rvn".into())
                .unwrap();
            *quick_pc = Some(engine.0.state.pc);
            input.send(PlayerInput::QuickLoad);
            phase.0 = 6;
            phase.1 = now;
        }
        12 if now - phase.1 > 1.0
            && capture
                .thumbnails
                .handles
                .contains_key("@save/thumbnail_1.png") =>
        {
            shots
                .save_screenshot_to_disk(window, qa.dir.join("save-with-thumbnail.png"))
                .unwrap();
            save.active = false;
            next.set(VnState::Waiting);
            phase.0 = 11;
            phase.1 = now;
        }
        6 | 8 if confirm.pending == Some((0, SaveMenuMode::Load)) && now - phase.1 > 1.0 => {
            let yes = phase.0 == 8;
            if let Some((_, i, t)) = buttons.iter().find(|(b, _, _)| b.0 == yes) {
                capture.pointer.position = Some(t.translation().truncate());
                if *i == Interaction::Hovered {
                    shots
                        .save_screenshot_to_disk(
                            window,
                            qa.dir.join(format!("quick-confirmation-{}.png", phase.0)),
                        )
                        .unwrap();
                    capture
                        .pointer
                        .buttons
                        .push(bevy::input::ButtonState::Pressed);
                    phase.2 = true;
                    phase.0 += 1;
                    phase.1 = now;
                }
            }
        }
        7 if confirm.pending.is_none() && *state.get() == VnState::Waiting => {
            assert_eq!(*quick_pc, Some(engine.0.state.pc));
            input.send(PlayerInput::QuickLoad);
            phase.0 = 8;
            phase.1 = now;
        }
        9 if confirm.pending.is_none()
            && confirm.approved.is_none()
            && *state.get() == VnState::Waiting =>
        {
            assert_eq!(*quick_pc, Some(engine.0.state.pc));
            std::fs::write(qa.dir.join("quick-confirmation.txt"),"Quickload cancel and approve both verified; cancellation preserves the active dialogue.").unwrap();
            exit.send(AppExit::Success);
            phase.0 = 10;
        }
        _ => {}
    }
}
#[derive(Resource)]
struct Qa {
    dir: PathBuf,
    started: std::time::Instant,
    pc: usize,
    entered: f64,
    captured: BTreeSet<String>,
    choices: usize,
    finishing: Option<f64>,
    menu_step: usize,
    menus: bool,
    map_target: Option<usize>,
    mouse_down: bool,
    map_clicked: Option<usize>,
    slider_step: usize,
}
#[derive(Default)]
struct QuickQa {
    step: usize,
    pc: Option<usize>,
    last: f64,
}
fn drive(
    mut qa: ResMut<Qa>,
    state: Res<State<VnState>>,
    mut next: ResMut<NextState<VnState>>,
    engine: Res<VnEngine>,
    mut input: EventWriter<PlayerInput>,
    mut shots: ResMut<ScreenshotManager>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut exit: EventWriter<AppExit>,
    mut buttons: Query<(
        &crate::menu_documents::MenuElement,
        &mut Interaction,
        &Node,
        &GlobalTransform,
    )>,
    mut quick: Local<QuickQa>,
    map: Res<crate::resources::ImagemapState>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut window_values: Query<&mut Window>,
    mut mouse: EventWriter<bevy::input::mouse::MouseButtonInput>,
    sliders: Query<(&crate::menu_documents::MenuSlider, &Node, &GlobalTransform)>,
    choice_buttons: Query<
        (
            &crate::components::ChoiceButton,
            &Node,
            &GlobalTransform,
            &Interaction,
        ),
        Without<crate::menu_documents::MenuElement>,
    >,
) {
    let now = qa.started.elapsed().as_secs_f64();
    if std::env::var_os("RVN_QA_CUSTOM_CHOICES").is_some() {
        let _ = std::fs::write(
            qa.dir.join("state.txt"),
            format!(
                "state={:?} pc={} choices={} interaction={:?} clicked={:?} buttons={:?}",
                state.get(),
                engine.0.state.pc,
                qa.choices,
                engine.0.current_interaction(),
                qa.map_clicked,
                choice_buttons
                    .iter()
                    .map(|(b, _, t, i)| (b.0, t.translation(), *i))
                    .collect::<Vec<_>>()
            ),
        );
    }
    if qa.mouse_down {
        mouse.send(bevy::input::mouse::MouseButtonInput {
            button: MouseButton::Left,
            state: bevy::input::ButtonState::Released,
            window: windows.single(),
        });
        qa.mouse_down = false;
    }
    // Complete example routes have substantially more dialogue than tiny QA
    // fixtures; compositors may throttle a covered native window to one FPS.
    let timeout = if std::env::var_os("RVN_QA_ROUTE").is_some() {
        600.0
    } else {
        240.0
    };
    if now > timeout {
        error!(
            "QA timeout: state={:?}, pc={}, interaction={:?}",
            state.get(),
            engine.0.state.pc,
            engine.0.current_interaction()
        );
        exit.send(AppExit::error());
        return;
    }
    if *state.get() == VnState::Error {
        error!("QA script error");
        exit.send(AppExit::error());
        return;
    }
    if let Some(at) = qa.finishing {
        if now - at > 3.0 {
            exit.send(AppExit::Success);
        }
        return;
    }
    // Explicitly exercises real custom-button dispatch, not the title-state shortcut.
    if qa.menus && qa.menu_step < 5 {
        if qa.menu_step == 1 && qa.slider_step < 4 && std::env::var_os("RVN_QA_SLIDERS").is_some() {
            if let Some((_, node, transform)) =
                sliders.iter().find(|(s, _, _)| s.0 == "music_volume")
            {
                let center = transform.translation().truncate();
                let left = center.x - node.size().x * 0.5 + 12.0;
                let width = (node.size().x - 24.0).max(1.0);
                match qa.slider_step {
                    0 => {
                        window_values
                            .single_mut()
                            .set_cursor_position(Some(Vec2::new(left + width * 0.25, center.y)));
                    }
                    1 => {
                        mouse.send(bevy::input::mouse::MouseButtonInput {
                            button: MouseButton::Left,
                            state: bevy::input::ButtonState::Pressed,
                            window: windows.single(),
                        });
                    }
                    2 => {
                        window_values
                            .single_mut()
                            .set_cursor_position(Some(Vec2::new(left + width * 0.75, center.y)));
                    }
                    _ => {
                        mouse.send(bevy::input::mouse::MouseButtonInput {
                            button: MouseButton::Left,
                            state: bevy::input::ButtonState::Released,
                            window: windows.single(),
                        });
                    }
                }
                qa.slider_step += 1;
            }
            if now > 25.0 {
                error!("QA slider unavailable");
                exit.send(AppExit::error());
            }
            return;
        }
        if now > 25.0 {
            error!("QA menu action {} unavailable", qa.menu_step);
            exit.send(AppExit::error());
            return;
        }
        if now < 2.0 + qa.menu_step as f64 * 2.0 {
            return;
        }
        let action = [
            rvn_ui::Action::Settings,
            rvn_ui::Action::Back,
            rvn_ui::Action::Gallery,
            rvn_ui::Action::Back,
            rvn_ui::Action::NewGame,
        ][qa.menu_step]
            .clone();
        if let Some((_, mut interaction, _, _)) =
            buttons.iter_mut().find(|(e, _, _, _)| e.action == action)
        {
            let key = format!("menu_{}", qa.menu_step);
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join(format!("{key}.png")))
                .expect("QA menu screenshot");
            qa.captured.insert(key);
            *interaction = Interaction::Pressed;
            qa.menu_step += 1;
        }
        return;
    }
    if std::env::var_os("RVN_QA_QUICK_ACTIONS").is_some() && quick.step < 7 {
        if quick.pc.is_none() {
            if *state.get() != VnState::Waiting {
                return;
            }
            quick.pc = Some(engine.0.state.pc);
            quick.last = now;
        }
        if now - quick.last < 0.7 {
            return;
        }
        if quick.step == 6 {
            assert_eq!(
                quick.pc,
                Some(engine.0.state.pc),
                "Quick controls advanced the story"
            );
            std::fs::write(qa.dir.join("quick-controls.txt"),"QuickSave / History / Back / Menu / Resume / QuickLoad: unchanged narrative position").unwrap();
            quick.step += 1;
            return;
        }
        let action = [
            rvn_ui::Action::QuickSave,
            rvn_ui::Action::History,
            rvn_ui::Action::Back,
            rvn_ui::Action::ToggleMenu,
            rvn_ui::Action::Resume,
            rvn_ui::Action::QuickLoad,
        ][quick.step]
            .clone();
        if let Some((_, interaction, _, transform)) =
            buttons.iter_mut().find(|(e, _, _, _)| e.action == action)
        {
            window_values
                .single_mut()
                .set_cursor_position(Some(transform.translation().truncate()));
            if *interaction == Interaction::Hovered {
                shots
                    .save_screenshot_to_disk(
                        windows.single(),
                        qa.dir.join(format!("quick_{}.png", quick.step)),
                    )
                    .unwrap();
                mouse.send(bevy::input::mouse::MouseButtonInput {
                    button: MouseButton::Left,
                    state: bevy::input::ButtonState::Pressed,
                    window: windows.single(),
                });
                qa.mouse_down = true;
                quick.step += 1;
                quick.last = now;
            }
        }
        if now > 45.0 {
            panic!("Quick control unavailable at step {}", quick.step);
        }
        return;
    }
    let preview_cycle = std::env::var_os("RVN_QA_PREVIEW").is_some()
        && qa.choices > 0
        && qa.pc != engine.0.state.pc
        && matches!(
            engine.0.current_interaction(),
            Ok(Some(rvn_core::Interaction::Choice { .. }))
        );
    if *state.get() == VnState::Finished
        || (engine.0.is_finished() && qa.choices > 0)
        || preview_cycle
    {
        info!(
            "QA playthrough finished, {} captured views",
            qa.captured.len()
        );
        std::fs::write(
            qa.dir.join("finished.txt"),
            format!(
                "Finished with {} choices and {} captured views\n",
                qa.choices,
                qa.captured.len()
            ),
        )
        .expect("QA report");
        qa.finishing = Some(now);
        return;
    }
    if *state.get() == VnState::TitleScreen {
        if qa.menus {
            if now > 20.0 {
                error!("QA NewGame did not leave the title menu");
                exit.send(AppExit::error());
            }
            return;
        }
        if now > 2.0 && qa.captured.insert("00_title".into()) {
            shots
                .save_screenshot_to_disk(windows.single(), qa.dir.join("00_title.png"))
                .expect("QA screenshot");
        }
        if now > 3.0 {
            next.set(VnState::Stepping);
        }
        return;
    }
    if *state.get() != VnState::Waiting {
        return;
    }
    if qa.pc != engine.0.state.pc {
        qa.pc = engine.0.state.pc;
        qa.entered = now;
        if std::env::var_os("RVN_QA_CUSTOM_CHOICES").is_some() {
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(qa.dir.join("dialogue-trace.txt"))
            {
                let _ = writeln!(file, "pc={} {:?}", qa.pc, engine.0.current_interaction());
            }
        }
        input.send(PlayerInput::SkipTypewriter);
        return;
    }
    let Ok(Some(interaction)) = engine.0.current_interaction() else {
        return;
    };
    if matches!(interaction, rvn_core::Interaction::Imagemap { .. }) {
        if let Some(target) = qa.map_target {
            if map.source_w <= 0.0 || map.source_h <= 0.0 {
                return;
            }
            let Some(zone) = map.hotspots.get(target) else {
                error!("QA imagemap target absent");
                exit.send(AppExit::error());
                return;
            };
            let world = Vec3::new(
                (zone.area.x1 + zone.area.x2) as f32 * 0.5 / map.source_w * 1280.0 - 640.0,
                360.0 - (zone.area.y1 + zone.area.y2) as f32 * 0.5 / map.source_h * 720.0,
                0.0,
            );
            let (camera, transform) = cameras.single();
            if let Some(cursor) = camera.world_to_viewport(transform, world) {
                window_values.single_mut().set_cursor_position(Some(cursor));
            }
        }
    }
    let key = match &interaction {
        rvn_core::Interaction::Choice { .. } => format!("choice_{}", qa.choices),
        rvn_core::Interaction::Imagemap { .. } => "map".into(),
        _ => {
            if std::env::var_os("RVN_QA_SPRITE_PERSISTENCE").is_some() {
                format!("sprite_step_{}", qa.pc)
            } else if let Some(cg) = &engine.0.state.cinematic.current {
                format!("cg_{cg}")
            } else {
                format!(
                    "scene_{}_sprites_{}",
                    engine.0.state.background_image.replace(['/', '.'], "_"),
                    engine
                        .0
                        .state
                        .sprites
                        .values()
                        .filter(|s| s.visible)
                        .count()
                )
            }
        }
    };
    let unseen = !qa.captured.contains(&key);
    if now - qa.entered < if unseen { 1.6 } else { 0.1 } {
        return;
    }
    if unseen {
        shots
            .save_screenshot_to_disk(windows.single(), qa.dir.join(format!("{key}.png")))
            .expect("QA screenshot");
        qa.captured.insert(key);
        qa.entered = now;
        return;
    }
    match interaction {
        rvn_core::Interaction::Choice { .. } => {
            let selected = if let Ok(route) = std::env::var("RVN_QA_ROUTE") {
                route
                    .split(',')
                    .nth(qa.choices)
                    .expect("QA route exhausted")
                    .parse::<usize>()
                    .expect("QA route must contain comma-separated indices")
            } else {
                match qa.choices {
                    0 => 2,
                    1 => 0,
                    2 => 2,
                    _ => 0,
                }
            };
            if std::env::var_os("RVN_QA_CUSTOM_CHOICES").is_some() {
                if qa.map_clicked == Some(engine.0.state.pc) {
                    return;
                }
                if let Some((_, _, transform, interaction)) =
                    choice_buttons.iter().find(|(id, _, _, _)| id.0 == selected)
                {
                    window_values
                        .single_mut()
                        .set_cursor_position(Some(transform.translation().truncate()));
                    if *interaction != Interaction::Hovered {
                        return;
                    }
                    mouse.send(bevy::input::mouse::MouseButtonInput {
                        button: MouseButton::Left,
                        state: bevy::input::ButtonState::Pressed,
                        window: windows.single(),
                    });
                    qa.mouse_down = true;
                    qa.map_clicked = Some(engine.0.state.pc);
                    qa.choices += 1;
                }
            } else {
                qa.choices += 1;
                input.send(PlayerInput::Choose(selected));
            }
        }
        rvn_core::Interaction::Imagemap { .. } => {
            if qa.map_target.is_some() {
                if qa.map_clicked == Some(engine.0.state.pc) {
                    return;
                }
                qa.map_clicked = Some(engine.0.state.pc);
                mouse.send(bevy::input::mouse::MouseButtonInput {
                    button: MouseButton::Left,
                    state: bevy::input::ButtonState::Pressed,
                    window: windows.single(),
                });
                qa.mouse_down = true;
                qa.choices += 1;
            } else {
                input.send(PlayerInput::Choose(1));
            }
        }
        _ => {
            input.send(PlayerInput::Advance);
        }
    }
}
