use bevy::prelude::*;
use rvn_core::save::SaveManager;
use rvn_parser::Transition;

use crate::components::DialogueText;
use crate::project_paths::ProjectPaths;
use crate::resources::{
    ChoiceFocus, DialogueHistory, ImagemapState, MenuState, PersistentDataResource,
    ScriptErrorMessage, SkipMode, TypewriterState, VnEngine, VnRenderState, VnState,
};
use crate::systems::save_menu::{apply_loaded_game, record_resume_target, SaveMenuState};
use crate::systems::typewriter::apply_visible_sections;
use crate::vn_command::{PlayerInput, VnCommand};
use rvn_core::error::ScriptError;
use rvn_core::persistent::LastResumeTarget;
#[derive(bevy::ecs::system::SystemParam)]
pub struct InputSaveContext<'w> {
    accessibility: Res<'w, crate::accessibility::Accessibility>,
    paths: Res<'w, ProjectPaths>,
    confirmation: ResMut<'w, crate::systems::save_menu::SaveConfirmation>,
    thumbnails: ResMut<'w, crate::save_thumbnails::SaveThumbnails>,
}

/// The core stack includes the currently displayed narrative anchor. The game
/// facade skips that anchor, while actual UI transactions remain undoable.
pub(crate) fn can_rollback<R:rvn_core::Renderer>(engine:&rvn_core::Engine<R>)->bool {
    let entries=engine.history.entries();
    entries.last().is_some_and(|entry| if entry.display.is_some()&&entry.state.pc==engine.state.pc {entries.len()>1} else {true})
}
pub(crate) fn rollback_game<R:rvn_core::Renderer>(engine:&mut rvn_core::Engine<R>)->bool {
    if !can_rollback(engine){return false;}
    let current=if engine.history.entries().last().is_some_and(|entry|entry.display.is_some()&&entry.state.pc==engine.state.pc) {
        engine.history.pop()
    } else {None};
    if engine.rollback(){true} else {
        if let Some(entry)=current{engine.history.push(entry.state,entry.display);}
        false
    }
}
pub(crate) fn present_rollback(engine:&mut VnEngine,render:&mut VnRenderState,imagemap:&mut ImagemapState,
    focus:&mut ChoiceFocus,typing:&mut TypewriterState,events:&mut EventWriter<VnCommand>) {
    render.choice_options.clear();imagemap.clear();focus.clear();typing.skip();
    let mut pending=engine.0.renderer.take_pending();
    for command in &mut pending {
        match command {
            VnCommand::SetBackground{transition,..}|VnCommand::ShowSprite{transition,..}
                |VnCommand::HideSprite{transition,..}|VnCommand::MoveSprite{transition,..}=>*transition=Transition::None,
            _=>{}
        }
    }
    for command in pending{events.send(command);}
}

pub fn input_system(
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut scroll_events: EventReader<bevy::input::mouse::MouseWheel>,
    windows: Query<&Window>,
    render_state: Res<VnRenderState>,
    imagemap_state: Res<ImagemapState>,
    tw_state: Res<TypewriterState>,
    mut choice_focus: ResMut<ChoiceFocus>,
    mut skip_mode: ResMut<SkipMode>,
    mut player_events: EventWriter<PlayerInput>,
    ui_buttons: Query<
        &Interaction,
        Or<(
            With<crate::menu_documents::MenuElement>,
            With<crate::menu_documents::UiInputBlocker>,
        )>,
    >,
    ui_scrollbars: Query<&Interaction, With<crate::menu_documents::Scrollbar>>,
    ui_lists: Query<(&Node, &GlobalTransform, &crate::menu_documents::MenuScroll)>,
    custom_menus: Res<crate::menu_documents::Menus>,
    programmable: Res<crate::programmable_ui::Screens>,
) {
    if programmable.pointer_consumed || programmable.keyboard_consumed {
        scroll_events.clear();
        return;
    }
    // A UI activation must not also advance dialogue or select an imagemap zone.
    if ui_buttons
        .iter()
        .chain(ui_scrollbars.iter())
        .any(|i| *i == Interaction::Pressed)
    {
        scroll_events.clear();
        return;
    }
    // ── Escape → menu ────────────────────────────────────────────────────────
    if keys.just_pressed(KeyCode::Escape) {
        player_events.send(PlayerInput::ToggleMenu);
        return;
    }

    if keys.just_pressed(KeyCode::F5) {
        player_events.send(PlayerInput::QuickSave);
        return;
    }

    if keys.just_pressed(KeyCode::F6) {
        player_events.send(PlayerInput::QuickLoad);
        return;
    }
    // A modal interface owns story activation; save/menu shortcuts remain usable.
    if programmable.modal() {
        scroll_events.clear();
        return;
    }

    // ── Scroll haut / ArrowUp hors-choix → historique ────────────────────────
    let mut scrolled_up = false;
    let over_scroll = windows
        .get_single()
        .ok()
        .and_then(|w| w.cursor_position())
        .is_some_and(|cursor| {
            ui_lists.iter().any(|(node, transform, scroll)| {
                let min = transform.translation().truncate() - node.size() * 0.5;
                scroll.content_height > node.size().y
                    && cursor.cmpge(min).all()
                    && cursor.cmplt(min + node.size()).all()
            })
        });
    for event in scroll_events.read() {
        if event.y > 0.0 && !over_scroll {
            scrolled_up = true;
        }
    }

    // ── Rollback ─────────────────────────────────────────────────────────────
    if mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Backspace) {
        player_events.send(PlayerInput::Rollback);
        return;
    }

    // ── Imagemap : clic souris uniquement ────────────────────────────────────
    if imagemap_state.active {
        if mouse.just_pressed(MouseButton::Left) {
            if let Ok(win) = windows.get_single() {
                if let Some(pos) = win.cursor_position() {
                    let world = cameras.get_single().ok().and_then(|(camera, transform)| {
                        camera.viewport_to_world_2d(transform, pos)
                    });
                    if let Some(idx) = world.and_then(|p| imagemap_state.hit_test_world(p)) {
                        player_events.send(PlayerInput::Choose(idx));
                    }
                }
            }
        }
        return;
    }

    // ── Choix ─────────────────────────────────────────────────────────────────
    let n = render_state.choice_options.len();
    if n > 0 {
        if !custom_menus.choices_interactive() {
            choice_focus.clear();
            return;
        }
        let digit_keys = [
            (KeyCode::Digit1, 0usize),
            (KeyCode::Digit2, 1),
            (KeyCode::Digit3, 2),
            (KeyCode::Digit4, 3),
            (KeyCode::Digit5, 4),
            (KeyCode::Digit6, 5),
            (KeyCode::Digit7, 6),
            (KeyCode::Digit8, 7),
            (KeyCode::Digit9, 8),
        ];
        for (key, idx) in &digit_keys {
            if *idx < n && keys.just_pressed(*key) {
                choice_focus.clear();
                player_events.send(PlayerInput::Choose(*idx));
                return;
            }
        }

        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        if keys.just_pressed(KeyCode::ArrowDown) || (keys.just_pressed(KeyCode::Tab) && !shift) {
            choice_focus.move_down(n);
            return;
        }

        // ArrowUp / Shift+Tab → focus précédent
        // Note : ArrowUp sans choix actif ouvre l'historique, donc on le gère
        // uniquement quand un choix est affiché.
        if keys.just_pressed(KeyCode::ArrowUp) || (keys.just_pressed(KeyCode::Tab) && shift) {
            choice_focus.move_up(n);
            return;
        }

        // Enter / Space → confirme le focus ou le premier choix
        if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
            let idx = choice_focus.0.unwrap_or(0);
            choice_focus.clear();
            player_events.send(PlayerInput::Choose(idx));
            return;
        }

        return;
    }

    // ── Hors-choix : ArrowUp → historique ────────────────────────────────────
    if scrolled_up || keys.just_pressed(KeyCode::ArrowUp) {
        player_events.send(PlayerInput::OpenHistory);
        return;
    }

    // ── Toggle skip mode (Ctrl) ──────────────────────────────────────
    if keys.just_pressed(KeyCode::ControlLeft) || keys.just_pressed(KeyCode::ControlRight) {
        skip_mode.active = !skip_mode.active;
    }

    // ── Auto-advance when skip mode is active ──
    if skip_mode.active {
        player_events.send(PlayerInput::Advance);
    }

    // ── Avancer / skip typewriter ─────────────────────────────────────────────
    if !mouse.just_pressed(MouseButton::Left)
        && !keys.just_pressed(KeyCode::Space)
        && !keys.just_pressed(KeyCode::Enter)
    {
        return;
    }

    if !tw_state.is_done() {
        player_events.send(PlayerInput::SkipTypewriter);
        return;
    }

    player_events.send(PlayerInput::Advance);
}

pub fn menu_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut player_events: EventWriter<PlayerInput>,
    source: Option<Res<crate::source_menus::SourceMenus>>,
) {
    if source.as_deref().is_some_and(|source|source.modal()) {return;}
    if keys.just_pressed(KeyCode::Escape) {
        player_events.send(PlayerInput::ToggleMenu);
    }
}

pub fn player_input_system(
    mut events: EventReader<PlayerInput>,
    mut engine: ResMut<VnEngine>,
    mut next_state: ResMut<NextState<VnState>>,
    current_state: Res<State<VnState>>,
    mut menu_state: ResMut<MenuState>,
    save_menu_state: Res<SaveMenuState>,
    mut render_state: ResMut<VnRenderState>,
    mut imagemap_state: ResMut<ImagemapState>,
    mut tw_state: ResMut<TypewriterState>,
    mut choice_focus: ResMut<ChoiceFocus>,
    mut history: ResMut<DialogueHistory>,
    mut persistent: ResMut<PersistentDataResource>,
    mut vn_events: EventWriter<VnCommand>,
    mut error_msg: ResMut<ScriptErrorMessage>,
    mut saves: InputSaveContext,
    mut dialogue_text_query: Query<&mut Text, With<DialogueText>>,
) {
    let project_paths = &saves.paths;
    if saves.accessibility.blocked {
        events.clear();
        return;
    }
    for input in events.read() {
        if saves.confirmation.active() {
            continue;
        }
        if engine.0.interface_is_modal()
            && matches!(
                input,
                PlayerInput::Advance | PlayerInput::Choose(_) | PlayerInput::SkipTypewriter
            )
        {
            continue;
        }
        match input {
            PlayerInput::Advance => {
                if let Err(e) = engine.0.advance_dialogue() {
                    let script_err = ScriptError::from_runtime(&e);
                    error!("{script_err}");
                    eprintln!("\n{script_err}");
                    error_msg.0 = script_err.to_string();
                    next_state.set(VnState::Error);
                    return;
                }
                next_state.set(VnState::Stepping);
            }

            PlayerInput::Choose(idx) => {
                if let Err(e) = engine.0.submit_selection(*idx) {
                    let script_err = ScriptError::from_runtime(&e);
                    error!("{script_err}");
                    eprintln!("\n{script_err}");
                    error_msg.0 = script_err.to_string();
                    next_state.set(VnState::Error);
                    return;
                }
                render_state.choice_options.clear();
                imagemap_state.clear();
                choice_focus.clear();
                next_state.set(VnState::Stepping);
            }

            PlayerInput::SkipTypewriter => {
                tw_state.skip();
                if let Ok(mut text) = dialogue_text_query.get_single_mut() {
                    apply_visible_sections(&mut text, &tw_state);
                }
            }

            PlayerInput::Rollback => {
                if rollback_game(&mut engine.0) {
                    present_rollback(&mut engine,&mut render_state,&mut imagemap_state,&mut choice_focus,&mut tw_state,&mut vn_events);
                    next_state.set(VnState::Waiting);
                }
            }

            PlayerInput::ToggleMenu => {
                if save_menu_state.active {
                    return;
                }
                let state = current_state.get();
                if *state == VnState::Menu {
                    let ret = menu_state.return_to.take().unwrap_or(VnState::Waiting);
                    next_state.set(ret);
                } else if *state != VnState::Animating && *state != VnState::Finished {
                    menu_state.return_to = Some(state.clone());
                    next_state.set(VnState::Menu);
                }
            }

            PlayerInput::OpenHistory => {
                if *current_state.get() == VnState::Waiting {
                    next_state.set(VnState::History);
                }
            }

            PlayerInput::QuickSave => {
                match SaveManager::new(
                    &project_paths.saves,
                    crate::systems::save_menu::SUPPORTED_SLOTS,
                ) {
                    Ok(mgr) => {
                        if let Err(e) = mgr.save_quicksave(
                            &engine.0.state,
                            "Quicksave".to_string(),
                            "script.rvn".to_string(),
                        ) {
                            error!("[quick_save] échec: {e}");
                        } else {
                            if let Ok(data) = mgr.load_quicksave() {
                                record_resume_target(
                                    &mut persistent,
                                    LastResumeTarget::quicksave(data.timestamp),
                                );
                                saves.thumbnails.request_resume(
                                    rvn_core::save::ResumeSlot::Quick,
                                    &engine,
                                    data,
                                );
                            }
                            info!("[quick_save] sauvegarde rapide écrite");
                        }
                    }
                    Err(e) => error!("[quick_save] SaveManager indisponible: {e}"),
                }
            }

            PlayerInput::QuickLoad => {
                let operation = (0, crate::systems::save_menu::SaveMenuMode::Load);
                let approved = saves.confirmation.approved == Some(operation);
                if approved {
                    saves.confirmation.approved = None;
                }
                if !approved {
                    if saves.confirmation.pending.is_some() {
                        return;
                    }
                    if SaveManager::new(
                        &project_paths.saves,
                        crate::systems::save_menu::SUPPORTED_SLOTS,
                    )
                    .ok()
                    .is_some_and(|m| m.load_quicksave().is_ok())
                    {
                        saves.confirmation.pending = Some(operation);
                        saves.confirmation.quick_return = Some(current_state.get().clone());
                        next_state.set(VnState::Menu);
                    }
                    return;
                }
                match SaveManager::new(
                    &project_paths.saves,
                    crate::systems::save_menu::SUPPORTED_SLOTS,
                ) {
                    Ok(mgr) => match mgr.load_quicksave() {
                        Ok(data) => {
                            match engine.0.load_data(data) {
                                Ok(rvn_core::engine::LoadCompatibility::LegacyUnchecked) => warn!("Ancienne sauvegarde : compatibilité après modification de l’histoire non vérifiable"),
                                Ok(_) => {},
                                Err(error) => { error!("Chargement refusé : {error}"); return; }
                            }
                            apply_loaded_game(
                                &mut engine,
                                &mut render_state,
                                &mut imagemap_state,
                                &mut tw_state,
                                &mut history,
                                &mut vn_events,
                            );
                            choice_focus.clear();
                            if let Ok(data) = mgr.load_quicksave() {
                                record_resume_target(
                                    &mut persistent,
                                    LastResumeTarget::quicksave(data.timestamp),
                                );
                            }
                            next_state.set(VnState::Waiting);
                            info!("[quick_load] sauvegarde rapide chargée");
                        }
                        Err(e) => info!("[quick_load] aucune quicksave chargeable: {e}"),
                    },
                    Err(e) => error!("[quick_load] SaveManager indisponible: {e}"),
                }
            }
        }
    }
}

#[cfg(test)]
mod rollback_tests {
    use super::*;
    fn game(source:&str)->rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
        let mut engine=rvn_core::Engine::new(rvn_parser::parse(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        engine.step_until_interaction().unwrap();engine
    }
    #[test]
    fn one_game_rollback_reaches_previous_dialogue_and_does_not_undo_a_counter_frame() {
        let mut engine=game("init{set counter=0}\n\"First\"\n\"Second\"");
        assert!(!can_rollback(&engine));engine.advance_dialogue().unwrap();engine.step_until_interaction().unwrap();let second=engine.state.pc;
        engine.state.vars.insert("counter".into(),rvn_parser::Value::Int(1));
        assert!(can_rollback(&engine));assert!(rollback_game(&mut engine));assert!(engine.state.pc<second);assert_eq!(engine.state.vars["counter"],rvn_parser::Value::Int(0));assert!(!can_rollback(&engine));
    }
    #[test]
    fn real_narrative_ui_transaction_is_restored_without_skipping_its_frame() {
        let mut engine=game("init{set counter=0}\nhandler increment(event){set counter=counter+1}\nscreen inventory(){return component(\"increment\",\"button\",{\"events\":{\"click\":\"increment\"}},[])}\nlabel start\nui.open(\"inventory\",[],false,0)\n\"Current\"");
        let pc=engine.state.pc;engine.interface_event(rvn_core::ui::UiInput{screen:"inventory".into(),element:"increment".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}).unwrap();
        assert!(engine.history.entries().last().unwrap().display.is_none());assert!(can_rollback(&engine));assert!(rollback_game(&mut engine));
        assert_eq!(engine.state.pc,pc);assert_eq!(engine.state.vars["counter"],rvn_parser::Value::Int(0));assert_eq!(engine.state.ui.screens[0].name,"inventory");assert!(!can_rollback(&engine));
    }
    #[test]
    fn rejected_previous_ui_state_keeps_story_rng_and_complete_history() {
        let mut engine=game("screen inventory(){return component(\"root\",\"text\",{},[])}\nlabel start\nui.open(\"inventory\",[],false,0)\n\"First\"\n\"Second\"");
        engine.advance_dialogue().unwrap();engine.step_until_interaction().unwrap();
        let current=engine.history.pop().unwrap();let mut target=engine.history.pop().unwrap();target.state.ui.screens[0].name="closed_unknown".into();engine.history.push(target.state,target.display);engine.history.push(current.state,current.display);
        let before=serde_json::to_value(&engine.state).unwrap();let history:Vec<_>=engine.history.entries().iter().map(|entry|serde_json::to_value(&entry.state).unwrap()).collect();
        assert!(!rollback_game(&mut engine));assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);
        assert_eq!(engine.history.entries().iter().map(|entry|serde_json::to_value(&entry.state).unwrap()).collect::<Vec<_>>(),history);
    }
}
