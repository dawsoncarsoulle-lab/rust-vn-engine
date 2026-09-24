use rvn_parser::{Hotspot, Position, Script, Statement, Transition, Value};
use std::collections::HashMap;

use crate::error::RuntimeError;
use crate::eval::{eval_bool, eval_expr, eval_interpolated, EvalError};
use crate::locale::LocaleManager;
use crate::renderer::Renderer;
use crate::rollback::{HistoryDisplay, RollbackHistory};
use crate::save::{SaveData, SaveManager};
use crate::types::{CinematicState, GameState, MusicState, SpriteState, TypewriterState};

/// Interaction actuellement proposée par le moteur.
///
/// Le renderer doit seulement afficher cette interaction puis renvoyer l'input
/// utilisateur via `advance_dialogue`, `submit_choice` ou `submit_hotspot`.
/// Cela garde la logique narrative dans `rvn_core` et évite que les renderers
/// réimplémentent chacun une partie du comportement des choix/imagemaps.
#[derive(Debug, Clone)]
pub enum Interaction {
    Dialogue {
        character: Option<String>,
        text: String,
    },
    Choice {
        options: Vec<String>,
    },
    Imagemap {
        background: String,
        hover_image: Option<String>,
        hotspots: Vec<Hotspot>,
    },
}

// ─── FLATTEN AST ─────────────────────────────────────────────────────────────

pub fn flatten_ast(script: &mut Script, extra: &mut Vec<Statement>, counter: &mut usize) {
    flatten_with_returns(script, extra, counter, &mut Vec::new());
}

fn flatten_with_returns(
    script: &mut Script,
    extra: &mut Vec<Statement>,
    counter: &mut usize,
    returns: &mut Vec<usize>,
) {
    for stmt in script.iter_mut() {
        match stmt {
            Statement::Use { .. } | Statement::Init { .. } => {}
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                flatten_with_returns(then_branch, extra, counter, returns);
                flatten_with_returns(else_branch, extra, counter, returns);
                if !then_branch.is_empty() {
                    *counter += 1;
                    let target = format!("__internal_then_{}", counter);
                    let mut block = std::mem::take(then_branch);
                    block.push(Statement::Return);
                    extra.push(Statement::Label {
                        name: target.clone(),
                    });
                    extra.extend(block);
                    returns.push(extra.len() - 1);
                    *then_branch = vec![Statement::Call { target }];
                }
                if !else_branch.is_empty() {
                    *counter += 1;
                    let target = format!("__internal_else_{}", counter);
                    let mut block = std::mem::take(else_branch);
                    block.push(Statement::Return);
                    extra.push(Statement::Label {
                        name: target.clone(),
                    });
                    extra.extend(block);
                    returns.push(extra.len() - 1);
                    *else_branch = vec![Statement::Call { target }];
                }
            }
            Statement::Choice { options } => {
                for opt in options.iter_mut() {
                    flatten_with_returns(&mut opt.body, extra, counter, returns);
                    if !opt.body.is_empty() {
                        *counter += 1;
                        let target = format!("__internal_choice_{}", counter);
                        let mut block = std::mem::take(&mut opt.body);
                        block.push(Statement::Return);
                        extra.push(Statement::Label {
                            name: target.clone(),
                        });
                        extra.extend(block);
                        returns.push(extra.len() - 1);
                        opt.body = vec![Statement::Call { target }];
                    }
                }
            }
            Statement::Imagemap { hotspots, .. } => {
                for hs in hotspots.iter_mut() {
                    flatten_with_returns(&mut hs.body, extra, counter, returns);
                    if !hs.body.is_empty() {
                        *counter += 1;
                        let target = format!("__internal_hotspot_{}", counter);
                        let mut block = std::mem::take(&mut hs.body);
                        block.push(Statement::Return);
                        extra.push(Statement::Label {
                            name: target.clone(),
                        });
                        extra.extend(block);
                        returns.push(extra.len() - 1);
                        hs.body = vec![Statement::Call { target }];
                    }
                }
            }
            _ => {}
        }
    }
}

// ─── HELPERS LOCALE ───────────────────────────────────────────────────────────

/// Convertit un InterpolatedText en clé de locale (avec [varname] pour les vars).
fn expr_to_display(expr: &rvn_parser::Expr) -> String {
    use rvn_parser::{BinOpKind, Expr};
    match expr {
        Expr::Int(n) => n.to_string(),
        Expr::Float(f) => f.to_string(),
        Expr::Bool(b) => b.to_string(),
        Expr::Str(s) => s.clone(),
        Expr::Var(n) => n.clone(),
        Expr::Neg(e) => format!("-{}", expr_to_display(e)),
        Expr::Not(e) => format!("not {}", expr_to_display(e)),
        Expr::And(l, r) => format!("{} and {}", expr_to_display(l), expr_to_display(r)),
        Expr::Or(l, r) => format!("{} or {}", expr_to_display(l), expr_to_display(r)),
        Expr::BinOp { op, left, right } => {
            let op_str = match op {
                BinOpKind::Add => "+",
                BinOpKind::Sub => "-",
                BinOpKind::Mul => "*",
                BinOpKind::Div => "/",
                BinOpKind::Eq => "==",
                BinOpKind::Ne => "!=",
                BinOpKind::Lt => "<",
                BinOpKind::Le => "<=",
                BinOpKind::Gt => ">",
                BinOpKind::Ge => ">=",
            };
            format!(
                "{} {} {}",
                expr_to_display(left),
                op_str,
                expr_to_display(right)
            )
        }
        Expr::Call { name, args } => {
            let args_str: Vec<String> = args.iter().map(expr_to_display).collect();
            format!("{}({})", name, args_str.join(", "))
        }
        Expr::ListLit(items) => {
            let parts: Vec<String> = items.iter().map(expr_to_display).collect();
            format!("[{}]", parts.join(", "))
        }
        Expr::Index { target, index } => {
            format!("{}[{}]", expr_to_display(target), expr_to_display(index))
        }
    }
}

fn text_to_locale_key(text: &rvn_parser::InterpolatedText) -> String {
    use rvn_parser::TextSegment;
    text.0
        .iter()
        .map(|seg| match seg {
            TextSegment::Lit(s) => s.clone(),
            TextSegment::Interp(expr) => format!("[{}]", expr_to_display(expr)),
        })
        .collect()
}

// ─── MOTEUR ──────────────────────────────────────────────────────────────────

/// Input state populated by the renderer for script-level input queries.
#[derive(Debug, Clone, Default)]
pub struct InputState {
    pub last_key_pressed: Option<String>,
    pub mouse_clicked: bool,
    pub mouse_x: f32,
    pub mouse_y: f32,
}

pub struct Engine<R: Renderer> {
    pub script: Script,
    label_table: HashMap<String, usize>,
    // Lowering adds calls for branches. Their synthetic returns pop one frame;
    // an authored return must unwind those frames and return to the real caller.
    branch_returns: std::collections::HashSet<usize>,
    pub state: GameState,
    pub renderer: R,
    pub history: RollbackHistory,
    /// Gestionnaire de localisation. None = pas de i18n (tests headless).
    pub locale: Option<LocaleManager>,
    /// Variables persistantes (préfixe `persistent.`). Survivent aux
    /// save/load et entre playthroughs. Stockées dans PersistentData.
    pub persistent_vars: HashMap<String, Value>,
    /// Input state for script queries (key_pressed, mouse_clicked, etc.)
    pub input_state: InputState,
    /// Active timer (duration_secs, action_string, elapsed_secs).
    pub active_timer: Option<(f32, String, f32)>,
}

impl<R: Renderer> Engine<R> {
    pub fn new(
        mut script: Script,
        renderer: R,
        rollback_depth: usize,
    ) -> Result<Self, RuntimeError> {
        let mut extra = Vec::new();
        let mut counter = 0;
        let mut returns = Vec::new();
        flatten_with_returns(&mut script, &mut extra, &mut counter, &mut returns);

        script.push(Statement::Jump {
            target: "__script_end".to_string(),
        });
        let branch_returns = returns.into_iter().map(|pc| pc + script.len()).collect();
        script.extend(extra);
        script.push(Statement::Label {
            name: "__script_end".to_string(),
        });

        Self::from_prepared_script(script, renderer, rollback_depth, branch_returns)
    }

    /// Start a clean game from this engine's already lowered script. Passing
    /// `self.script` back through `new` would lower choice calls twice and reuse
    /// internal labels, creating recursive calls instead of the original branch.
    pub fn fresh(&self, renderer: R, rollback_depth: usize) -> Result<Self, RuntimeError> {
        Self::from_prepared_script(
            self.script.clone(),
            renderer,
            rollback_depth,
            self.branch_returns.clone(),
        )
    }

    fn from_prepared_script(
        script: Script,
        renderer: R,
        rollback_depth: usize,
        branch_returns: std::collections::HashSet<usize>,
    ) -> Result<Self, RuntimeError> {
        let label_table = script
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                if let Statement::Label { name } = s {
                    Some((name.clone(), i))
                } else {
                    None
                }
            })
            .collect();

        let mut engine = Self {
            script,
            label_table,
            branch_returns,
            state: GameState {
                last_dialogue: None,
                pc: 0,
                current_interactive_pc: 0,
                background_image: String::new(),
                vars: HashMap::new(),
                call_stack: Vec::new(),
                last_transition: Transition::None,
                sprites: HashMap::new(),
                cinematic: CinematicState::default(),
                music: MusicState::new(),
                typewriter: TypewriterState::new(),
            },
            renderer,
            history: RollbackHistory::new(rollback_depth),
            locale: None,
            persistent_vars: HashMap::new(),
            input_state: InputState::default(),
            active_timer: None,
        };

        engine.run_init_blocks()?;
        Ok(engine)
    }

    // ── Utilitaires internes ──────────────────────────────────────────────────

    fn run_init_blocks(&mut self) -> Result<(), RuntimeError> {
        let mut init_blocks = Vec::new();
        for (i, stmt) in self.script.iter().enumerate() {
            if let Statement::Init { body } = stmt {
                init_blocks.push((i, body.clone()));
            } else if !matches!(stmt, Statement::Label { .. }) {
                break;
            }
        }
        for (idx, body) in init_blocks {
            self.exec_block(&body)?;
            if self.state.pc == idx {
                self.state.pc = idx + 1;
            }
        }
        Ok(())
    }

    fn resolve(&self, name: &str) -> Result<usize, RuntimeError> {
        self.label_table.get(name).copied().ok_or_else(|| {
            RuntimeError::no_stmt(
                crate::error::RuntimeErrorKind::UndefinedLabel(name.to_string()),
                self.state.pc,
            )
        })
    }

    /// Traduit une string via le LocaleManager si présent.
    fn translate<'b>(&'b self, text: &'b str) -> &'b str {
        self.locale
            .as_ref()
            .map(|l| l.translate(text))
            .unwrap_or(text)
    }

    /// Convertit un EvalError en RuntimeError avec contexte d'exécution.
    /// Appelé avec `|e| self.eval_err(e, stmt_label)` aux sites d'erreur.
    fn eval_err(&self, e: EvalError, stmt_label: &str) -> RuntimeError {
        RuntimeError::new(
            crate::error::RuntimeErrorKind::EvalError(e),
            self.state.pc,
            stmt_label.to_string(),
        )
    }

    fn exec_show(
        &mut self,
        id: &str,
        emotion: Option<String>,
        position: Option<Position>,
        transition: Transition,
    ) {
        let resolved = position.unwrap_or_else(|| {
            self.state
                .sprites
                .get(id)
                .map(|s| s.position.clone())
                .unwrap_or(Position::Center)
        });
        let from = self.state.sprites.get(id).cloned();
        self.state.last_transition = transition.clone();
        self.renderer.show_sprite(
            id,
            emotion.as_deref(),
            &resolved,
            &transition,
            from.as_ref(),
        );
        self.state
            .sprites
            .insert(id.to_string(), SpriteState::new(emotion, resolved));
    }

    fn exec_hide(&mut self, id: &str, transition: Transition) -> Result<(), RuntimeError> {
        let from = self.state.sprites.get(id).filter(|s| s.visible).cloned();
        // Explicit removal is idempotent, including before the first appearance.
        let Some(from) = from else { return Ok(()) };
        self.state.last_transition = transition.clone();
        self.renderer.hide_sprite(id, &transition, &from);
        if let Some(s) = self.state.sprites.get_mut(id) {
            s.visible = false;
        }
        Ok(())
    }

    fn exec_move(
        &mut self,
        id: &str,
        position: Position,
        transition: Transition,
    ) -> Result<(), RuntimeError> {
        let from = self
            .state
            .sprites
            .get(id)
            .filter(|s| s.visible)
            .cloned()
            .ok_or_else(|| {
                RuntimeError::no_stmt(
                    crate::error::RuntimeErrorKind::SpriteNotVisible(id.to_string()),
                    self.state.pc,
                )
            })?;
        self.state.last_transition = transition.clone();
        self.renderer.move_sprite(id, &position, &transition, &from);
        if let Some(s) = self.state.sprites.get_mut(id) {
            s.position = position;
        }
        Ok(())
    }

    // ── step() ───────────────────────────────────────────────────────────────

    pub fn step(&mut self) -> Result<(), RuntimeError> {
        if self.is_finished() {
            return Ok(());
        }
        loop {
            if self.is_finished() {
                return Ok(());
            }
            let stmt = self.script[self.state.pc].clone();
            if Self::is_interactive(&stmt) {
                self.record_interaction_snapshot(&stmt)?;
                return self.exec_interactive(stmt);
            } else {
                self.exec_silent(stmt)?;
            }
        }
    }

    pub fn rollback(&mut self) -> bool {
        let Some(entry) = self.history.pop() else {
            return false;
        };
        self.state = entry.state;
        self.renderer.restore_screen(&self.state);
        // Re-resolve the restored interaction in the currently selected language.
        // The cached display is only a fallback for an unresolvable legacy entry.
        if let Ok(Some(interaction)) = self.current_interaction() {
            if matches!(interaction, Interaction::Choice { .. }) {
                if let Ok(Some(dialogue)) = self.last_dialogue_interaction() {
                    self.render_interaction(dialogue);
                }
            }
            self.render_interaction(interaction);
            return true;
        }
        if let Some(display) = &entry.display {
            match display {
                HistoryDisplay::Dialogue { character, text } => {
                    self.renderer.show_dialogue(character.as_deref(), text);
                }
                HistoryDisplay::Choice { options } => {
                    self.renderer.show_choice(options);
                }
                HistoryDisplay::Imagemap {
                    background,
                    hover_image,
                    hotspots,
                } => {
                    self.renderer
                        .show_imagemap(background, hover_image.as_deref(), hotspots);
                }
            }
        }
        true
    }

    fn render_interaction(&mut self, interaction: Interaction) {
        match interaction {
            Interaction::Dialogue { character, text } => {
                self.renderer.show_dialogue(character.as_deref(), &text);
            }
            Interaction::Choice { options } => {
                self.renderer.show_choice(&options);
            }
            Interaction::Imagemap {
                background,
                hover_image,
                hotspots,
            } => {
                self.renderer
                    .show_imagemap(&background, hover_image.as_deref(), &hotspots);
            }
        }
    }

    pub fn can_rollback(&self) -> bool {
        self.history.can_rollback()
    }

    fn is_interactive(stmt: &Statement) -> bool {
        matches!(
            stmt,
            Statement::Dialogue { .. } | Statement::Choice { .. } | Statement::Imagemap { .. }
        )
    }

    /// Crée un HistoryDisplay avec les textes déjà évalués (interpolation résolue).
    fn make_display_resolved(
        &self,
        stmt: &Statement,
    ) -> Result<Option<HistoryDisplay>, RuntimeError> {
        Ok(match stmt {
            Statement::Dialogue { character_id, text } => {
                let resolved = eval_interpolated(text, &self.vars_for_eval())
                    .map_err(|e| self.eval_err(e, "Dialogue (interpolation)"))?;
                Some(HistoryDisplay::Dialogue {
                    character: character_id.clone(),
                    text: resolved,
                })
            }
            Statement::Choice { options } => {
                let mut resolved = Vec::new();
                for opt in options {
                    resolved.push(
                        eval_interpolated(&opt.label, &self.vars_for_eval())
                            .map_err(|e| self.eval_err(e, "Choice (label interpolation)"))?,
                    );
                }
                Some(HistoryDisplay::Choice { options: resolved })
            }
            Statement::Imagemap {
                background,
                hover_image,
                hotspots,
            } => Some(HistoryDisplay::Imagemap {
                background: background.clone(),
                hover_image: hover_image.clone(),
                hotspots: hotspots.clone(),
            }),
            _ => None,
        })
    }

    fn resolve_dialogue_text(
        &self,
        text: &rvn_parser::InterpolatedText,
    ) -> Result<String, RuntimeError> {
        let template_key = text_to_locale_key(text);
        let translated_tmpl = self.translate(&template_key).to_string();
        if translated_tmpl == template_key {
            eval_interpolated(text, &self.vars_for_eval())
                .map_err(|e| self.eval_err(e, "Dialogue (interpolation)"))
        } else {
            match rvn_parser::parse_interpolated_str(&translated_tmpl) {
                Ok(t) => eval_interpolated(&t, &self.vars_for_eval())
                    .map_err(|e| self.eval_err(e, "Dialogue (traduction + interpolation)")),
                Err(_) => Ok(translated_tmpl),
            }
        }
    }

    fn resolve_choice_labels(
        &self,
        options: &[rvn_parser::ChoiceOption],
    ) -> Result<Vec<String>, RuntimeError> {
        let mut labels = Vec::new();
        for opt in options {
            let template_key = text_to_locale_key(&opt.label);
            let translated = self.translate(&template_key).to_string();
            let final_label = if translated == template_key {
                eval_interpolated(&opt.label, &self.vars_for_eval())
                    .map_err(|e| self.eval_err(e, "Choice (label interpolation)"))?
            } else {
                match rvn_parser::parse_interpolated_str(&translated) {
                    Ok(t) => eval_interpolated(&t, &self.vars_for_eval())
                        .map_err(|e| self.eval_err(e, "Choice (label traduction)"))?,
                    Err(_) => translated,
                }
            };
            labels.push(final_label);
        }
        Ok(labels)
    }

    fn record_interaction_snapshot(&mut self, stmt: &Statement) -> Result<(), RuntimeError> {
        if matches!(stmt, Statement::Dialogue { .. }) {
            self.state.last_dialogue = Some(crate::types::DialogueSnapshot {
                pc: self.state.pc,
                vars: self.vars_for_eval(),
            });
        }
        self.state.current_interactive_pc = self.state.pc;
        let display = self.make_display_resolved(stmt)?;
        self.history.push(self.state.clone(), display);
        Ok(())
    }

    fn exec_interactive(&mut self, stmt: Statement) -> Result<(), RuntimeError> {
        match stmt {
            Statement::Dialogue { character_id, text } => {
                let template_key = text_to_locale_key(&text);
                let translated_tmpl = self.translate(&template_key).to_string();
                let final_text = if translated_tmpl == template_key {
                    eval_interpolated(&text, &self.vars_for_eval())
                        .map_err(|e| self.eval_err(e, "Dialogue (interpolation)"))?
                } else {
                    match rvn_parser::parse_interpolated_str(&translated_tmpl) {
                        Ok(t) => eval_interpolated(&t, &self.vars_for_eval()).map_err(|e| {
                            self.eval_err(e, "Dialogue (traduction + interpolation)")
                        })?,
                        Err(_) => translated_tmpl,
                    }
                };
                self.renderer
                    .show_dialogue(character_id.as_deref(), &final_text);
                self.state.pc += 1;
            }
            Statement::Choice { options } => {
                // Filter out options whose condition evaluates to false.
                let vars = self.vars_for_eval();
                let active: Vec<&rvn_parser::ChoiceOption> = options
                    .iter()
                    .filter(|opt| match &opt.condition {
                        Some(cond) => eval_bool(cond, &vars).unwrap_or(false),
                        None => true,
                    })
                    .collect();
                drop(vars);
                // If no options are active, skip the choice entirely.
                if active.is_empty() {
                    self.state.pc += 1;
                    return Ok(());
                }
                // Evaluate the choice labels before displaying them.  If localisation
                // provides a translation for the template key use it, otherwise
                // perform interpolation on the original label.
                let mut labels = Vec::new();
                for opt in &active {
                    let template_key = text_to_locale_key(&opt.label);
                    let translated = self.translate(&template_key).to_string();
                    let final_label = if translated == template_key {
                        eval_interpolated(&opt.label, &self.vars_for_eval())
                            .map_err(|e| self.eval_err(e, "Choice (label interpolation)"))?
                    } else {
                        match rvn_parser::parse_interpolated_str(&translated) {
                            Ok(t) => eval_interpolated(&t, &self.vars_for_eval())
                                .map_err(|e| self.eval_err(e, "Choice (label traduction)"))?,
                            Err(_) => translated,
                        }
                    };
                    labels.push(final_label);
                }
                let selected = self.renderer.show_choice(&labels);
                let idx = selected.min(active.len().saturating_sub(1));
                let body = active[idx].body.clone();
                if !body.is_empty() {
                    self.exec_silent(body[0].clone())?;
                } else {
                    self.state.pc += 1;
                }
            }
            Statement::Imagemap {
                background,
                hover_image,
                hotspots,
            } => {
                // If there are no hotspots, skip the imagemap and advance the PC.
                if hotspots.is_empty() {
                    self.state.pc += 1;
                    return Ok(());
                }
                let selected =
                    self.renderer
                        .show_imagemap(&background, hover_image.as_deref(), &hotspots);
                // Clamp to a valid index to avoid panics if the renderer returns an
                // out-of-range selection.
                let idx = selected.min(hotspots.len().saturating_sub(1));
                let body = hotspots[idx].body.clone();
                if !body.is_empty() {
                    self.exec_silent(body[0].clone())?;
                } else {
                    self.state.pc += 1;
                }
            }
            _ => unreachable!("exec_interactive appelé sur un statement non-interactif"),
        }
        Ok(())
    }

    fn exec_silent(&mut self, stmt: Statement) -> Result<(), RuntimeError> {
        match stmt {
            Statement::Use { .. } | Statement::Init { .. } | Statement::Label { .. } => {
                self.state.pc += 1;
            }
            Statement::Jump { target } => {
                self.state.pc = self.resolve(&target)?;
            }
            Statement::Call { target } => {
                self.state.call_stack.push(self.state.pc + 1);
                self.state.pc = self.resolve(&target)?;
            }
            Statement::Return => {
                if !self.branch_returns.contains(&self.state.pc) {
                    while self.state.call_stack.last().is_some_and(|pc| {
                        pc.checked_sub(1)
                            .and_then(|caller| self.script.get(caller))
                            .is_some_and(|s| {
                                matches!(
                                    s,
                                    Statement::If { .. }
                                        | Statement::Choice { .. }
                                        | Statement::Imagemap { .. }
                                )
                            })
                    }) {
                        self.state.call_stack.pop();
                    }
                }
                self.state.pc = self.state.call_stack.pop().ok_or(RuntimeError::no_stmt(
                    crate::error::RuntimeErrorKind::ReturnWithoutCall,
                    self.state.pc,
                ))?;
            }
            Statement::Config { key, value } => {
                if key == "bg" {
                    self.state.background_image = value.clone();
                }
                self.state.pc += 1;
            }
            Statement::Scene {
                background,
                transition,
            } => {
                self.state.background_image = background.clone();
                self.state.last_transition = transition.clone();
                self.renderer.set_background(&background, &transition);
                self.state.pc += 1;
            }
            Statement::CinematicShow { id, transition } => {
                self.state.cinematic.current = Some(id.clone());
                self.state.cinematic.transition = transition.clone();
                self.renderer.show_cinematic(&id, transition.as_deref());
                self.state.pc += 1;
            }
            Statement::CinematicHide { transition } => {
                self.state.cinematic.current = None;
                self.state.cinematic.transition = transition.clone();
                self.renderer.hide_cinematic(transition.as_deref());
                self.state.pc += 1;
            }
            Statement::UnlockEnding { id } => {
                self.renderer.unlock_ending(&id);
                self.state.pc += 1;
            }
            Statement::ShowSprite {
                character_id,
                emotion,
                position,
                transition,
            } => {
                self.exec_show(&character_id, emotion, position, transition);
                self.state.pc += 1;
            }
            Statement::HideSprite {
                character_id,
                transition,
            } => {
                self.exec_hide(&character_id, transition)?;
                self.state.pc += 1;
            }
            Statement::MoveSprite {
                character_id,
                position,
                transition,
            } => {
                self.exec_move(&character_id, position, transition)?;
                self.state.pc += 1;
            }
            Statement::SpriteAnimate {
                character_id,
                animation,
                params,
            } => {
                self.renderer
                    .animate_sprite(&character_id, &animation, &params);
                self.state.pc += 1;
            }
            Statement::SpriteStopAnimation { character_id } => {
                self.renderer.stop_sprite_animation(&character_id);
                self.state.pc += 1;
            }
            Statement::SpriteEffect {
                character_id,
                flip_x,
                flip_y,
                scale,
                rotation,
                tint,
            } => {
                let tint = tint
                    .map(|text| {
                        let parsed = rvn_parser::parse_interpolated_str(&text).map_err(|_| {
                            self.eval_err(
                                EvalError::TypeMismatch {
                                    op: "tint".into(),
                                    left: "color expression".into(),
                                    right: text.clone(),
                                },
                                "SpriteEffect",
                            )
                        })?;
                        eval_interpolated(&parsed, &self.vars_for_eval())
                            .map_err(|e| self.eval_err(e, "SpriteEffect"))
                    })
                    .transpose()?;
                self.renderer.set_sprite_effect(
                    &character_id,
                    flip_x,
                    flip_y,
                    scale,
                    rotation,
                    tint.as_deref(),
                );
                self.state.pc += 1;
            }
            Statement::MethodCall {
                target,
                method,
                arg,
                transition,
            } => {
                self.state.last_transition = transition.clone();
                println!(
                    "{target}.{method}({})  [{transition}]",
                    arg.as_deref().unwrap_or("")
                );
                self.state.pc += 1;
            }
            Statement::CharacterCreate { id, display_name } => {
                println!("personnage : {id} (\"{display_name}\")");
                self.state.pc += 1;
            }
            Statement::SetVar { name, value } => {
                let val = eval_expr(&value, &self.vars_for_eval())
                    .map_err(|e| self.eval_err(e, &format!("SetVar {{ name: {:?} }}", name)))?;
                if Self::is_persistent(name.as_str()) {
                    self.persistent_vars.insert(name, val);
                } else {
                    self.state.vars.insert(name, val);
                }
                self.state.pc += 1;
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let branch = if eval_bool(&condition, &self.vars_for_eval())
                    .map_err(|e| self.eval_err(e, "If (condition)"))?
                {
                    then_branch
                } else {
                    else_branch
                };
                if !branch.is_empty() {
                    self.exec_silent(branch[0].clone())?;
                } else {
                    self.state.pc += 1;
                }
            }
            Statement::MusicPlay { file, transition } => {
                // Prefix music files with the `music/` directory if not already
                // specified.  This mirrors the path resolution used in the CLI
                // checker so that `music.play("theme.ogg")` resolves to
                // `assets/music/theme.ogg`.
                let mut resolved = file.clone();
                if !resolved.starts_with("music/") {
                    resolved = format!("music/{}", resolved);
                }
                let previous = self.state.music.current_file.clone();
                self.state.music.current_file = Some(resolved.clone());
                self.renderer
                    .music_play(&resolved, &transition, previous.as_deref());
                self.state.pc += 1;
            }
            Statement::MusicStop { transition } => {
                self.state.music.current_file = None;
                self.renderer.music_stop(&transition);
                self.state.pc += 1;
            }
            Statement::MusicVolume { level } => {
                self.state.music.volume = level;
                self.renderer.music_set_volume(level);
                self.state.pc += 1;
            }
            Statement::SfxPlay { file, transition } => {
                // Prefix sound effect files with the `sfx/` directory if not already
                // specified, following the same convention as the CLI.  This allows
                // `sfx.play("click.wav")` to resolve to `assets/sfx/click.wav`.
                let mut resolved = file.clone();
                if !resolved.starts_with("sfx/") {
                    resolved = format!("sfx/{}", resolved);
                }
                self.renderer.sfx_play(&resolved, &transition);
                self.state.pc += 1;
            }
            Statement::SfxStop { file, transition } => {
                // Apply the same `sfx/` prefix resolution as in SfxPlay when stopping
                // a sound effect so that `sfx.stop("click.wav")` resolves to the
                // correct asset path.
                let mut resolved = file.clone();
                if !resolved.starts_with("sfx/") {
                    resolved = format!("sfx/{}", resolved);
                }
                self.renderer.sfx_stop(&resolved, &transition);
                self.state.pc += 1;
            }
            Statement::VoicePlay { file } => {
                self.renderer.voice_play(&file);
                self.state.pc += 1;
            }
            Statement::VoiceStop => {
                self.renderer.voice_stop();
                self.state.pc += 1;
            }
            Statement::Timer {
                duration_secs,
                action,
            } => {
                self.active_timer = Some((duration_secs, action.clone(), 0.0));
                self.state.pc += 1;
            }
            Statement::TimerCancel => {
                self.active_timer = None;
                self.state.pc += 1;
            }
            Statement::TypewriterSet { enabled } => {
                self.state.typewriter.enabled = enabled;
                self.renderer
                    .set_typewriter_config(self.state.typewriter.effective_speed());
                self.state.pc += 1;
            }
            Statement::TypewriterSpeed { chars_per_sec } => {
                self.state.typewriter.chars_per_sec = chars_per_sec;
                self.state.typewriter.enabled = chars_per_sec > 0;
                self.renderer
                    .set_typewriter_config(self.state.typewriter.effective_speed());
                self.state.pc += 1;
            }
            Statement::Dialogue { .. } | Statement::Choice { .. } | Statement::Imagemap { .. } => {
                unreachable!("exec_silent appelé sur un statement interactif")
            }
        }
        Ok(())
    }

    fn exec_block(&mut self, stmts: &[Statement]) -> Result<(), RuntimeError> {
        let saved_pc = self.state.pc;
        for stmt in stmts {
            self.state.pc = self.script.len();
            self.exec_silent(stmt.clone())?;
        }
        self.state.pc = saved_pc;
        Ok(())
    }

    // ── API publique ──────────────────────────────────────────────────────────

    /// Avance le script jusqu'à la prochaine interaction ou jusqu'à la fin.
    ///
    /// Contrairement à `step()`, cette méthode ne demande pas au renderer de
    /// bloquer pour récupérer une réponse. Elle retourne simplement l'interaction
    /// courante, déjà résolue côté moteur. C'est l'API adaptée aux renderers
    /// événementiels comme Bevy.
    pub fn step_until_interaction(&mut self) -> Result<Option<Interaction>, RuntimeError> {
        loop {
            self.step_silent()?;
            if self.is_finished() {
                return Ok(None);
            }

            let stmt = self.script[self.state.pc].clone();
            match &stmt {
                // Ces cas devraient être signalés par `rvn check`, mais le moteur
                // reste robuste et ne bloque pas l'UI si le script les contient.
                Statement::Choice { options } if options.is_empty() => {
                    self.state.pc += 1;
                    continue;
                }
                Statement::Imagemap { hotspots, .. } if hotspots.is_empty() => {
                    self.state.pc += 1;
                    continue;
                }
                _ if Self::is_interactive(&stmt) => {
                    self.record_interaction_snapshot(&stmt)?;
                    return self.current_interaction();
                }
                _ => return Ok(None),
            }
        }
    }

    /// Retourne l'interaction actuellement pointée par le PC, sans modifier l'état.
    pub fn current_interaction(&self) -> Result<Option<Interaction>, RuntimeError> {
        let Some(stmt) = self.script.get(self.state.pc) else {
            return Ok(None);
        };

        match stmt {
            Statement::Dialogue { character_id, text } => Ok(Some(Interaction::Dialogue {
                character: character_id.clone(),
                text: self.resolve_dialogue_text(text)?,
            })),
            Statement::Choice { options } => Ok(Some(Interaction::Choice {
                options: self.resolve_choice_labels(options)?,
            })),
            Statement::Imagemap {
                background,
                hover_image,
                hotspots,
            } => Ok(Some(Interaction::Imagemap {
                background: background.clone(),
                hover_image: hover_image.clone(),
                hotspots: hotspots.clone(),
            })),
            _ => Ok(None),
        }
    }

    /// Re-render a recorded dialogue in the selected language without replaying
    /// commands or substituting the current value of ordinary story variables.
    pub fn localized_dialogue_history(
        &self,
    ) -> Result<Vec<(Option<String>, String)>, RuntimeError> {
        self.history
            .entries()
            .iter()
            .filter_map(|entry| {
                if !matches!(
                    self.script.get(entry.state.pc),
                    Some(Statement::Dialogue { .. })
                ) {
                    return None;
                }
                Some(
                    self.resolve_recorded_dialogue(&crate::types::DialogueSnapshot {
                        pc: entry.state.pc,
                        vars: entry
                            .state
                            .last_dialogue
                            .as_ref()
                            .map(|d| d.vars.clone())
                            .unwrap_or_else(|| entry.state.vars.clone()),
                    }),
                )
            })
            .collect()
    }

    pub fn last_dialogue_interaction(&self) -> Result<Option<Interaction>, RuntimeError> {
        self.state
            .last_dialogue
            .as_ref()
            .map(|snapshot| {
                self.resolve_recorded_dialogue(snapshot)
                    .map(|(character, text)| Interaction::Dialogue { character, text })
            })
            .transpose()
    }

    fn resolve_recorded_dialogue(
        &self,
        snapshot: &crate::types::DialogueSnapshot,
    ) -> Result<(Option<String>, String), RuntimeError> {
        let Some(Statement::Dialogue { character_id, text }) = self.script.get(snapshot.pc) else {
            return Ok((None, String::new()));
        };
        let key = text_to_locale_key(text);
        let translated = self.translate(&key);
        let template = if translated == key {
            text.clone()
        } else {
            match rvn_parser::parse_interpolated_str(translated) {
                Ok(template) => template,
                Err(_) => return Ok((character_id.clone(), translated.to_owned())),
            }
        };
        eval_interpolated(&template, &snapshot.vars)
            .map(|text| (character_id.clone(), text))
            .map_err(|e| self.eval_err(e, "History translation"))
    }

    /// Valide un dialogue affiché et avance au statement suivant.
    pub fn advance_dialogue(&mut self) -> Result<(), RuntimeError> {
        match self.script.get(self.state.pc) {
            Some(Statement::Dialogue { .. }) => {
                self.state.pc += 1;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Soumet un choix utilisateur au moteur.
    pub fn submit_choice(&mut self, selected: usize) -> Result<(), RuntimeError> {
        let Some(stmt) = self.script.get(self.state.pc).cloned() else {
            return Ok(());
        };
        let Statement::Choice { options } = stmt else {
            return Ok(());
        };

        if options.is_empty() {
            self.state.pc += 1;
            return Ok(());
        }

        let idx = selected.min(options.len().saturating_sub(1));
        let body = options[idx].body.clone();
        if !body.is_empty() {
            self.exec_silent(body[0].clone())?;
        } else {
            self.state.pc += 1;
        }
        Ok(())
    }

    /// Soumet un hotspot d'imagemap au moteur.
    pub fn submit_hotspot(&mut self, selected: usize) -> Result<(), RuntimeError> {
        let Some(stmt) = self.script.get(self.state.pc).cloned() else {
            return Ok(());
        };
        let Statement::Imagemap { hotspots, .. } = stmt else {
            return Ok(());
        };

        if hotspots.is_empty() {
            self.state.pc += 1;
            return Ok(());
        }

        let idx = selected.min(hotspots.len().saturating_sub(1));
        let body = hotspots[idx].body.clone();
        if !body.is_empty() {
            self.exec_silent(body[0].clone())?;
        } else {
            self.state.pc += 1;
        }
        Ok(())
    }

    /// Soumet une sélection générique. Utile côté UI, où un clic peut venir soit
    /// d'un choice soit d'une imagemap.
    pub fn submit_selection(&mut self, selected: usize) -> Result<(), RuntimeError> {
        match self.script.get(self.state.pc) {
            Some(Statement::Choice { .. }) => self.submit_choice(selected),
            Some(Statement::Imagemap { .. }) => self.submit_hotspot(selected),
            _ => Ok(()),
        }
    }

    pub fn save(
        &self,
        manager: &SaveManager,
        slot: u32,
        label: String,
        script_name: String,
    ) -> Result<(), crate::save::SaveError> {
        manager.save(&self.state, slot, label, script_name)
    }

    pub fn load(&mut self, manager: &SaveManager, slot: u32) -> Result<(), crate::save::SaveError> {
        let data = manager.load(slot)?;
        self.load_data(data);
        Ok(())
    }

    pub fn load_data(&mut self, data: SaveData) {
        self.state = data.into_game_state();
        self.history.clear();
        self.renderer.restore_screen(&self.state);
        if matches!(
            self.current_interaction(),
            Ok(Some(Interaction::Choice { .. }))
        ) {
            if let Ok(Some(dialogue)) = self.last_dialogue_interaction() {
                self.render_interaction(dialogue);
            }
        }
        if let Ok(Some(interaction)) = self.current_interaction() {
            self.render_interaction(interaction);
        }
    }

    pub fn is_finished(&self) -> bool {
        self.state.pc >= self.script.len()
    }
    pub fn get_var(&self, name: &str) -> Option<&Value> {
        self.state.vars.get(name)
    }

    /// Returns a merged view of regular + persistent variables for evaluation.
    /// Persistent variables are keyed with the `persistent.` prefix.
    fn vars_for_eval(&self) -> HashMap<String, Value> {
        let mut merged = self.state.vars.clone();
        for (k, v) in &self.persistent_vars {
            merged.insert(k.clone(), v.clone());
        }
        // Inject input state for script queries.
        if let Some(key) = &self.input_state.last_key_pressed {
            merged.insert("__input_key".to_string(), Value::Str(key.clone()));
        }
        merged.insert(
            "__input_mouse_clicked".to_string(),
            Value::Bool(self.input_state.mouse_clicked),
        );
        merged.insert(
            "__input_mouse_x".to_string(),
            Value::Float(self.input_state.mouse_x),
        );
        merged.insert(
            "__input_mouse_y".to_string(),
            Value::Float(self.input_state.mouse_y),
        );
        merged
    }

    /// Whether a variable name targets the persistent store.
    fn is_persistent(name: &str) -> bool {
        name.starts_with("persistent.")
    }

    pub fn get_persistent_var(&self, name: &str) -> Option<&Value> {
        self.persistent_vars.get(name)
    }

    /// Load persistent script variables from a PersistentData snapshot.
    pub fn load_persistent_vars(&mut self, vars: &HashMap<String, Value>) {
        self.persistent_vars = vars.clone();
    }

    /// Export persistent script variables for storage in PersistentData.
    pub fn export_persistent_vars(&self) -> &HashMap<String, Value> {
        &self.persistent_vars
    }

    /// Update the input state from the renderer. Called each frame before
    /// script execution to allow key_pressed() and mouse_clicked() queries.
    /// Advance the active timer by delta_secs. Returns the action string
    /// if the timer has elapsed, and clears the timer.
    pub fn tick_timer(&mut self, delta_secs: f32) -> Option<String> {
        if let Some((duration, action, elapsed)) = &mut self.active_timer {
            *elapsed += delta_secs;
            if *elapsed >= *duration {
                let action = action.clone();
                self.active_timer = None;
                return Some(action);
            }
        }
        None
    }

    /// Parse a timer action string ("jump label" or "call label") and execute it.
    pub fn execute_timer_action(&mut self, action: &str) -> Result<(), RuntimeError> {
        let parts: Vec<&str> = action.splitn(2, ' ').collect();
        if parts.len() == 2 {
            match parts[0] {
                "jump" => {
                    let target = parts[1].to_string();
                    self.state.pc = self.resolve(&target)?;
                }
                "call" => {
                    let target = parts[1].to_string();
                    let idx = self.resolve(&target)?;
                    self.state.call_stack.push(self.state.pc + 1);
                    self.state.pc = idx;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn update_input_state(
        &mut self,
        last_key: Option<String>,
        mouse_clicked: bool,
        mouse_x: f32,
        mouse_y: f32,
    ) {
        self.input_state.last_key_pressed = last_key;
        self.input_state.mouse_clicked = mouse_clicked;
        self.input_state.mouse_x = mouse_x;
        self.input_state.mouse_y = mouse_y;
    }
    pub fn get_sprite(&self, id: &str) -> Option<&SpriteState> {
        self.state.sprites.get(id)
    }

    pub fn step_silent(&mut self) -> Result<(), RuntimeError> {
        loop {
            if self.is_finished() {
                return Ok(());
            }
            let stmt = self.script[self.state.pc].clone();
            if Self::is_interactive(&stmt) {
                return Ok(());
            }
            self.exec_silent(stmt)?;
        }
    }

    pub fn peek_statement(&self) -> Option<&Statement> {
        self.script.get(self.state.pc)
    }
}
