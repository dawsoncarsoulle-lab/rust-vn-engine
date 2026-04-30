use bevy::prelude::*;
use rvn_parser::Transition;

use super::{CHOICE_MARGIN_ABOVE_BOX, TEXTBOX_H, WIN_H, WIN_W};
use crate::resources::{
    ChoiceFocus, ImagemapState, MenuState, ScriptErrorMessage, TypewriterState, VnEngine,
    VnRenderState, VnState,
};
use crate::vn_command::{PlayerInput, VnCommand};
use rvn_core::error::ScriptError;

pub fn input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut scroll_events: EventReader<bevy::input::mouse::MouseWheel>,
    windows: Query<&Window>,
    render_state: Res<VnRenderState>,
    imagemap_state: Res<ImagemapState>,
    tw_state: Res<TypewriterState>,
    mut choice_focus: ResMut<ChoiceFocus>,
    mut player_events: EventWriter<PlayerInput>,
) {
    // ── Escape → menu ────────────────────────────────────────────────────────
    if keys.just_pressed(KeyCode::Escape) {
        player_events.send(PlayerInput::ToggleMenu);
        return;
    }

    // ── Scroll haut / ArrowUp hors-choix → historique ────────────────────────
    let mut scrolled_up = false;
    for event in scroll_events.read() {
        if event.y > 0.0 {
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
                let (win_w, win_h) = (win.width(), win.height());
                if let Some(pos) = win.cursor_position() {
                    if let Some(idx) = imagemap_state.hit_test(pos.x, pos.y, win_w, win_h) {
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

        // Clic gauche → hit-test sur les boutons (dimensions réelles)
        if mouse.just_pressed(MouseButton::Left) {
            if let Ok(win) = windows.get_single() {
                let win_w = win.width();
                let win_h = win.height();
                if let Some(pos) = win.cursor_position() {
                    let cursor_x = pos.x;
                    let cursor_y_ui = win_h - pos.y;
                    let btn_w = (win_w * 0.5).min(640.0);
                    let btn_h = 52.0_f32;
                    let gap = 10.0_f32;
                    // La textbox est positionnée en bas ; on scale sa hauteur
                    // proportionnellement si la fenêtre est redimensionnée.
                    let textbox_h = TEXTBOX_H * (win_h / WIN_H);
                    let base_bottom = textbox_h + CHOICE_MARGIN_ABOVE_BOX;
                    let btn_left = (win_w - btn_w) / 2.0;

                    for i in 0..n {
                        let btn_bottom = base_bottom + i as f32 * (btn_h + gap);
                        if cursor_x >= btn_left
                            && cursor_x <= btn_left + btn_w
                            && cursor_y_ui >= btn_bottom
                            && cursor_y_ui <= btn_bottom + btn_h
                        {
                            choice_focus.clear();
                            player_events.send(PlayerInput::Choose(i));
                            return;
                        }
                    }
                }
            }
        }

        return;
    }

    // ── Hors-choix : ArrowUp → historique ────────────────────────────────────
    if scrolled_up || keys.just_pressed(KeyCode::ArrowUp) {
        player_events.send(PlayerInput::OpenHistory);
        return;
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
) {
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
    mut render_state: ResMut<VnRenderState>,
    mut imagemap_state: ResMut<ImagemapState>,
    mut tw_state: ResMut<TypewriterState>,
    mut choice_focus: ResMut<ChoiceFocus>,
    mut vn_events: EventWriter<VnCommand>,
    mut error_msg: ResMut<ScriptErrorMessage>,
) {
    for input in events.read() {
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
            }

            PlayerInput::Rollback => {
                if engine.0.rollback() {
                    render_state.choice_options.clear();
                    imagemap_state.clear();
                    choice_focus.clear();
                    tw_state.skip();

                    let mut pending = engine.0.renderer.take_pending();
                    for cmd in pending.iter_mut() {
                        match cmd {
                            VnCommand::SetBackground { transition, .. } => {
                                *transition = Transition::None
                            }
                            VnCommand::ShowSprite { transition, .. } => {
                                *transition = Transition::None
                            }
                            VnCommand::HideSprite { transition, .. } => {
                                *transition = Transition::None
                            }
                            VnCommand::MoveSprite { transition, .. } => {
                                *transition = Transition::None
                            }
                            _ => {}
                        }
                    }
                    for cmd in pending {
                        vn_events.send(cmd);
                    }
                    next_state.set(VnState::Waiting);
                }
            }

            PlayerInput::ToggleMenu => {
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
        }
    }
}
