use rvn_parser::{Hotspot, Position, Script, Statement, Transition, Value};
use std::collections::HashMap;

use crate::error::RuntimeError;
use crate::eval::{EvalError, FunctionLibrary};
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
#[derive(Debug, Clone, PartialEq)]
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
    // Iteration state uses reserved engine variables, so existing save/rollback
    // snapshots capture it together with the story instead of losing a cursor.
    let mut index = 0;
    while index < script.len() {
        if let Statement::ForEach {
            name,
            collection,
            body,
        } = script[index].clone()
        {
            *counter += 1;
            let items = format!("__rvn_for_{}_items", counter);
            let cursor = format!("__rvn_for_{}_index", counter);
            let mut iteration = vec![
                Statement::SetVar {
                    name,
                    value: rvn_parser::Expr::Index {
                        target: Box::new(rvn_parser::Expr::Var(items.clone())),
                        index: Box::new(rvn_parser::Expr::Var(cursor.clone())),
                    },
                },
                Statement::SetVar {
                    name: cursor.clone(),
                    value: rvn_parser::Expr::BinOp {
                        op: rvn_parser::BinOpKind::Add,
                        left: Box::new(rvn_parser::Expr::Var(cursor.clone())),
                        right: Box::new(rvn_parser::Expr::Int(1)),
                    },
                },
            ];
            iteration.extend(body);
            let replacement = vec![
                Statement::SetVar {
                    name: items.clone(),
                    value: rvn_parser::Expr::Call {
                        name: "__rvn_iterable".into(),
                        args: vec![collection],
                    },
                },
                Statement::SetVar {
                    name: cursor.clone(),
                    value: rvn_parser::Expr::Int(0),
                },
                Statement::While {
                    condition: rvn_parser::Expr::BinOp {
                        op: rvn_parser::BinOpKind::Lt,
                        left: Box::new(rvn_parser::Expr::Var(cursor)),
                        right: Box::new(rvn_parser::Expr::Call {
                            name: "len".into(),
                            args: vec![rvn_parser::Expr::Var(items)],
                        }),
                    },
                    body: iteration,
                },
            ];
            script.splice(index..=index, replacement);
            index += 2;
        }
        index += 1;
    }
    for stmt in script.iter_mut() {
        match stmt {
            Statement::Use { .. } | Statement::Init { .. } => {}
            Statement::While { body, .. } => {
                flatten_with_returns(body, extra, counter, returns);
                *counter += 1;
                let target = format!("__internal_loop_{}", counter);
                let mut block = std::mem::take(body);
                block.push(Statement::Return);
                extra.push(Statement::Label {
                    name: target.clone(),
                });
                extra.extend(block);
                returns.push(extra.len() - 1);
                *body = vec![Statement::Call { target }];
            }
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
    functions: FunctionLibrary,
    ui_library: crate::ui::UiLibrary,
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
    /// Never restored from a save: late decoder callbacks from an old playback
    /// must not mutate a restored or replaced video with the same name.
    video_epoch: u64,
    /// Transient input generation; saved/restored component IDs do not revive
    /// a pointer capture from the previous state.
    interface_epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadCompatibility {
    Verified,
    /// Older saves have no story identity. Bounds are checked, but authors
    /// must not describe compatibility after a story edit as guaranteed.
    LegacyUnchecked,
}

fn story_identity(script: &Script) -> String {
    // Fixed, portable FNV-1a over the canonical AST encoding. This detects
    // ordinary story changes; it is deliberately not an authenticity check.
    let bytes = serde_json::to_vec(script).expect("RVN AST serialization is infallible");
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("rvn-prepared-2:{hash:016x}:{}", bytes.len())
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
        let mut engine = Self::from_prepared_script(
            self.script.clone(),
            renderer,
            rollback_depth,
            self.branch_returns.clone(),
        )?;
        engine.interface_epoch = self.interface_epoch.wrapping_add(1);
        Ok(engine)
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

        let functions = FunctionLibrary::from_script(&script).map_err(|error| {
            RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error), 0)
        })?;
        let identity = story_identity(&script);
        let ui_library = crate::ui::UiLibrary::from_script(&script).map_err(|error| {
            RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error), 0)
        })?;
        let mut engine = Self {
            script,
            functions,
            ui_library,
            label_table,
            branch_returns,
            state: GameState {
                accessibility: Default::default(),
                speech_requests: Vec::new(),
                videos: Default::default(),
                layered: Default::default(),
                motions: Default::default(),
                ui: Default::default(),
                story_identity: Some(identity),
                random: crate::random::RandomState::fresh(),
                display_random: Default::default(),
                display_random_pc: None,
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
            video_epoch: 0,
            interface_epoch: 0,
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
            } else if !matches!(
                stmt,
                Statement::Label { .. }
                    | Statement::Function { .. }
                    | Statement::Screen { .. }
                    | Statement::Handler { .. }
            ) {
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
    ) -> Result<(), RuntimeError> {
        let resolved = position.unwrap_or_else(|| {
            self.state
                .sprites
                .get(id)
                .map(|s| s.position.clone())
                .unwrap_or(Position::Center)
        });
        let from = self.state.sprites.get(id).cloned();
        let mut next = self.state.clone();
        next.last_transition = transition.clone();
        next.sprites.insert(
            id.to_string(),
            SpriteState::new(emotion.clone(), resolved.clone()),
        );
        self.commit_ui_state(next)?;
        self.renderer.show_sprite(
            id,
            emotion.as_deref(),
            &resolved,
            &transition,
            from.as_ref(),
        );
        Ok(())
    }

    fn exec_hide(&mut self, id: &str, transition: Transition) -> Result<(), RuntimeError> {
        let from = self.state.sprites.get(id).filter(|s| s.visible).cloned();
        // Explicit removal is idempotent, including before the first appearance.
        let Some(from) = from else { return Ok(()) };
        let mut next = self.state.clone();
        next.last_transition = transition.clone();
        next.motions.tracks.retain(|_,track|!matches!(&track.target,crate::motion::MotionTarget::Sprite{id:target}|crate::motion::MotionTarget::Layer{id:target,..} if target==id));
        if let Some(s) = next.sprites.get_mut(id) {
            s.visible = false;
        }
        self.commit_ui_state(next)?;
        self.renderer.hide_sprite(id, &transition, &from);
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
        let mut next = self.state.clone();
        next.last_transition = transition.clone();
        if let Some(s) = next.sprites.get_mut(id) {
            s.position = position.clone();
        }
        self.commit_ui_state(next)?;
        self.renderer.move_sprite(id, &position, &transition, &from);
        Ok(())
    }

    // ── step() ───────────────────────────────────────────────────────────────

    pub fn step(&mut self) -> Result<(), RuntimeError> {
        if self.state.motions.waiting.is_some() || self.state.videos.waiting.is_some() {
            return Ok(());
        }
        if self.is_finished() {
            return Ok(());
        }
        for _ in 0..crate::eval::MAX_COMPUTATION_STEPS {
            if self.is_finished() {
                return Ok(());
            }
            let stmt = self.script[self.state.pc].clone();
            if Self::is_interactive(&stmt) {
                self.record_interaction_snapshot(&stmt)?;
                return self.exec_interactive(stmt);
            } else {
                self.exec_silent(stmt)?;
                if self.state.motions.waiting.is_some() || self.state.videos.waiting.is_some() {
                    return Ok(());
                }
            }
        }
        Err(self.eval_err(
            EvalError::ExecutionLimit {
                limit: "100 000 instructions sans interaction",
            },
            "step",
        ))
    }

    pub fn rollback(&mut self) -> bool {
        let Some(entry) = self.history.pop() else {
            return false;
        };
        let Ok(views) = self.describe_interfaces(&entry.state) else {
            self.history.push(entry.state, entry.display);
            return false;
        };
        let Ok(motions) = self.describe_motions(&entry.state) else {
            self.history.push(entry.state, entry.display);
            return false;
        };
        let Ok(layered) = self.describe_layered_characters(&entry.state) else {
            self.history.push(entry.state, entry.display);
            return false;
        };
        let Ok(epoch) = self.next_video_epoch() else {
            self.history.push(entry.state, entry.display);
            return false;
        };
        let Ok(videos) = self.describe_videos(&entry.state, epoch) else {
            self.history.push(entry.state, entry.display);
            return false;
        };
        if entry.state.accessibility.validate().is_err() {
            self.history.push(entry.state, entry.display);
            return false;
        }
        if (!views.is_empty() && !self.renderer.supports_programmable_ui())
            || self.validate_canvas_renderer(&views).is_err()
        {
            self.history.push(entry.state, entry.display);
            return false;
        }
        let previous = self.state.clone();
        self.state = entry.state.clone();
        self.video_epoch = epoch;
        self.renderer.restore_screen(&self.state);
        if self.renderer.update_interfaces(&views).is_err()
            || self.renderer.update_layered_characters(&layered).is_err()
            || self.renderer.update_motions(&motions).is_err()
            || self.renderer.update_videos(&videos).is_err()
            || self
                .renderer
                .update_accessibility(&self.state.accessibility)
                .is_err()
        {
            self.history.push(entry.state, entry.display);
            self.state = previous;
            self.renderer.restore_screen(&self.state);
            if let Ok(views) = self.interface_views() {
                let _ = self.renderer.update_interfaces(&views);
            }
            let _ = self.refresh_motions();
            let _ = self.refresh_layered_characters();
            if let Ok(views) = self.video_views() {
                let _ = self.renderer.update_videos(&views);
            }
            let _ = self
                .renderer
                .update_accessibility(&self.state.accessibility);
            return false;
        }
        self.interface_epoch = self.interface_epoch.wrapping_add(1);
        if self.renderer.supports_accessibility() {
            let _ = self
                .renderer
                .accessibility_speech(&rvn_ui::accessibility::SpeechRequest::Stop);
        }
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

    fn interpolate_display(
        &self,
        text: &rvn_parser::InterpolatedText,
    ) -> crate::eval::EvalResult<String> {
        self.functions.interpolate_with_random(
            text,
            &self.vars_for_eval(),
            &mut self.state.display_random.clone(),
        )
    }

    /// Crée un HistoryDisplay avec les textes déjà évalués (interpolation résolue).
    fn make_display_resolved(
        &self,
        stmt: &Statement,
    ) -> Result<Option<HistoryDisplay>, RuntimeError> {
        Ok(match stmt {
            Statement::Dialogue { character_id, text } => {
                let resolved = self
                    .interpolate_display(text)
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
                        self.interpolate_display(&opt.label)
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
            self.interpolate_display(text)
                .map_err(|e| self.eval_err(e, "Dialogue (interpolation)"))
        } else {
            match rvn_parser::parse_interpolated_str(&translated_tmpl) {
                Ok(t) => self
                    .interpolate_display(&t)
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
                self.interpolate_display(&opt.label)
                    .map_err(|e| self.eval_err(e, "Choice (label interpolation)"))?
            } else {
                match rvn_parser::parse_interpolated_str(&translated) {
                    Ok(t) => self
                        .interpolate_display(&t)
                        .map_err(|e| self.eval_err(e, "Choice (label traduction)"))?,
                    Err(_) => translated,
                }
            };
            labels.push(final_label);
        }
        Ok(labels)
    }

    fn record_interaction_snapshot(&mut self, stmt: &Statement) -> Result<(), RuntimeError> {
        if self.state.display_random_pc != Some(self.state.pc) {
            self.state.display_random =
                crate::random::RandomState::seeded(self.state.random.next_u64());
            self.state.display_random_pc = Some(self.state.pc);
        }
        if matches!(stmt, Statement::Dialogue { .. }) {
            self.state.last_dialogue = Some(crate::types::DialogueSnapshot {
                pc: self.state.pc,
                vars: self.vars_for_eval(),
                random: self.state.display_random,
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
                    self.interpolate_display(&text)
                        .map_err(|e| self.eval_err(e, "Dialogue (interpolation)"))?
                } else {
                    match rvn_parser::parse_interpolated_str(&translated_tmpl) {
                        Ok(t) => self.interpolate_display(&t).map_err(|e| {
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
                let mut active = Vec::new();
                for option in &options {
                    let visible = match &option.condition {
                        Some(condition) => self
                            .functions
                            .eval_bool_with_random(
                                condition,
                                &vars,
                                &mut self.state.display_random.clone(),
                            )
                            .map_err(|error| self.eval_err(error, "Choice (condition)"))?,
                        None => true,
                    };
                    if visible {
                        active.push(option);
                    }
                }
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
                        self.interpolate_display(&opt.label)
                            .map_err(|e| self.eval_err(e, "Choice (label interpolation)"))?
                    } else {
                        match rvn_parser::parse_interpolated_str(&translated) {
                            Ok(t) => self
                                .interpolate_display(&t)
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
            Statement::Use { .. }
            | Statement::Init { .. }
            | Statement::Label { .. }
            | Statement::Function { .. }
            | Statement::Screen { .. }
            | Statement::Handler { .. } => {
                self.state.pc += 1;
            }
            Statement::FunctionReturn { .. } => {
                return Err(self.eval_err(
                    EvalError::InvalidFunction("return <valeur> hors d’une fonction".into()),
                    "FunctionReturn",
                ))
            }
            Statement::LocalVar { .. } => {
                return Err(self.eval_err(
                    EvalError::InvalidFunction("local hors d’une fonction ou gestionnaire".into()),
                    "LocalVar",
                ))
            }
            statement @ (Statement::UiOpen { .. }
            | Statement::UiClose { .. }
            | Statement::UiFocus { .. }
            | Statement::UiSetState { .. }
            | Statement::MotionPlay { .. }
            | Statement::MotionStop { .. }
            | Statement::CharacterCompose { .. }
            | Statement::CharacterAttributes { .. }
            | Statement::VideoPlay { .. }
            | Statement::VideoPause { .. }
            | Statement::VideoResume { .. }
            | Statement::VideoStop { .. }
            | Statement::VideoSkip { .. }
            | Statement::VideoSeek { .. }
            | Statement::VideoVolume { .. }
            | Statement::AccessibilityConfigure { .. }
            | Statement::AccessibilitySpeak { .. }
            | Statement::AccessibilityStop) => {
                let mut next = self.state.clone();
                let command = crate::eval::UiCommand::evaluate(
                    &self.functions,
                    &statement,
                    &next.vars,
                    &mut next.random,
                )
                .map_err(|error| self.eval_err(error, "Interface"))?;
                self.apply_ui_commands(&mut next, vec![command])?;
                self.commit_ui_state(next)?;
                self.state.pc += 1;
            }
            Statement::VideoWait { name } => {
                let mut next = self.state.clone();
                let Value::Str(name) = self
                    .functions
                    .eval_with_random(&name, &self.vars_for_eval(), &mut next.random)
                    .map_err(|error| self.eval_err(error, "Video wait"))?
                else {
                    return Err(self.eval_err(
                        EvalError::InvalidFunction("Video wait requires a player name".into()),
                        "Video wait",
                    ));
                };
                next.videos.wait(&name).map_err(|message| {
                    self.eval_err(EvalError::InvalidFunction(message), "Video wait")
                })?;
                next.pc += 1;
                self.commit_ui_state(next)?;
            }
            Statement::MotionWait { target } => {
                let mut random = self.state.random;
                let value = self
                    .functions
                    .eval_with_random(&target, &self.vars_for_eval(), &mut random)
                    .map_err(|error| self.eval_err(error, "Motion wait"))?;
                let Value::Str(key) = value else {
                    return Err(self.eval_err(
                        EvalError::InvalidFunction("Animation target must be a string".into()),
                        "Motion wait",
                    ));
                };
                crate::motion::MotionTarget::parse(&key).map_err(|message| {
                    self.eval_err(EvalError::InvalidFunction(message), "Motion wait")
                })?;
                if let Some(track) = self
                    .state
                    .motions
                    .tracks
                    .get(&key)
                    .filter(|track| track.running)
                {
                    if track
                        .definition
                        .validate()
                        .map_err(|message| {
                            self.eval_err(EvalError::InvalidFunction(message), "Motion wait")
                        })?
                        .seconds
                        .is_none()
                    {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction(
                                "Cannot wait for an endless animation; use a finite repeat count"
                                    .into(),
                            ),
                            "Motion wait",
                        ));
                    }
                    self.state.motions.waiting = Some(key);
                    self.state.current_interactive_pc = self.state.pc;
                } else {
                    self.state.pc += 1;
                }
                self.state.random = random;
            }
            Statement::While { condition, body } if self.state.pc < self.script.len() => {
                if self
                    .functions
                    .eval_bool_with_random(
                        &condition,
                        &self.vars_for_eval(),
                        &mut self.state.random,
                    )
                    .map_err(|error| self.eval_err(error, "While"))?
                {
                    let [Statement::Call { target }] = body.as_slice() else {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction("boucle narrative non préparée".into()),
                            "While",
                        ));
                    };
                    if self.state.call_stack.len() >= 128 {
                        return Err(self.eval_err(
                            EvalError::ExecutionLimit {
                                limit: "128 appels narratifs imbriqués",
                            },
                            "While",
                        ));
                    }
                    self.state.call_stack.push(self.state.pc);
                    self.state.pc = self.resolve(target)?;
                } else {
                    self.state.pc += 1;
                }
            }
            statement @ (Statement::While { .. } | Statement::ForEach { .. }) => {
                let original = self.vars_for_eval();
                let computed = self
                    .functions
                    .execute_with_random(&[statement], &original, &mut self.state.random)
                    .map_err(|error| self.eval_err(error, "Loop"))?;
                for (name, value) in computed {
                    if original.get(&name) != Some(&value) {
                        self.state.vars.insert(name, value);
                    }
                }
                self.state.pc += 1;
            }
            Statement::Jump { target } => {
                self.state.pc = self.resolve(&target)?;
            }
            Statement::Call { target } => {
                if self.state.call_stack.len() >= 128 {
                    return Err(self.eval_err(
                        EvalError::ExecutionLimit {
                            limit: "128 appels narratifs imbriqués",
                        },
                        "Call",
                    ));
                }
                let destination = self.resolve(&target)?;
                self.state.call_stack.push(self.state.pc + 1);
                self.state.pc = destination;
            }
            Statement::Return => {
                if !self.branch_returns.contains(&self.state.pc) {
                    while self.state.call_stack.last().is_some_and(|pc| {
                        matches!(self.script.get(*pc), Some(Statement::While { .. }))
                            || pc
                                .checked_sub(1)
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
                self.state.motions.tracks.remove("background");
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
                self.exec_show(&character_id, emotion, position, transition)?;
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
                        self.functions
                            .interpolate_with_random(
                                &parsed,
                                &self.vars_for_eval(),
                                &mut self.state.random,
                            )
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
                let mut random = self.state.random;
                let val = self
                    .functions
                    .eval_with_random(&value, &self.vars_for_eval(), &mut random)
                    .map_err(|e| self.eval_err(e, &format!("SetVar {{ name: {:?} }}", name)))?;
                if Self::is_persistent(name.as_str()) {
                    self.persistent_vars.insert(name, val);
                } else {
                    if self.state.ui.screens.is_empty() {
                        self.state.vars.insert(name, val);
                    } else {
                        let mut next = self.state.clone();
                        next.vars.insert(name, val);
                        next.random = random;
                        self.commit_ui_state(next)?;
                    }
                }
                self.state.random = random;
                self.state.pc += 1;
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let branch = if self
                    .functions
                    .eval_bool_with_random(
                        &condition,
                        &self.vars_for_eval(),
                        &mut self.state.random,
                    )
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
        let mut remaining = crate::eval::MAX_COMPUTATION_STEPS;
        loop {
            self.step_silent_bounded(&mut remaining)?;
            if self.state.videos.waiting.is_some() {
                return Ok(None);
            }
            if self.is_finished() {
                return Ok(None);
            }

            let stmt = self.script[self.state.pc].clone();
            match &stmt {
                // Ces cas devraient être signalés par `rvn check`, mais le moteur
                // reste robuste et ne bloque pas l'UI si le script les contient.
                Statement::Choice { options } if options.is_empty() => {
                    self.consume_step(&mut remaining)?;
                    self.state.pc += 1;
                    continue;
                }
                Statement::Imagemap { hotspots, .. } if hotspots.is_empty() => {
                    self.consume_step(&mut remaining)?;
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
        if self.state.videos.waiting.is_some() {
            return Ok(None);
        }
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
                        random: entry.state.display_random,
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
        self.functions
            .interpolate_with_random(&template, &snapshot.vars, &mut snapshot.random.clone())
            .map(|text| (character_id.clone(), text))
            .map_err(|e| self.eval_err(e, "History translation"))
    }

    /// Valide un dialogue affiché et avance au statement suivant.
    pub fn advance_dialogue(&mut self) -> Result<(), RuntimeError> {
        if self.state.videos.waiting.is_some() {
            return Ok(());
        }
        match self.script.get(self.state.pc) {
            Some(Statement::Dialogue { .. }) => {
                self.state.display_random_pc = None;
                self.state.pc += 1;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Soumet un choix utilisateur au moteur.
    pub fn submit_choice(&mut self, selected: usize) -> Result<(), RuntimeError> {
        if self.state.videos.waiting.is_some() {
            return Ok(());
        }
        let Some(stmt) = self.script.get(self.state.pc).cloned() else {
            return Ok(());
        };
        let Statement::Choice { options } = stmt else {
            return Ok(());
        };
        self.state.display_random_pc = None;

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
        if self.state.videos.waiting.is_some() {
            return Ok(());
        }
        let Some(stmt) = self.script.get(self.state.pc).cloned() else {
            return Ok(());
        };
        let Statement::Imagemap { hotspots, .. } = stmt else {
            return Ok(());
        };
        self.state.display_random_pc = None;

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
        self.load_data(data)?;
        Ok(())
    }

    pub fn load_data(
        &mut self,
        data: SaveData,
    ) -> Result<LoadCompatibility, crate::save::SaveError> {
        use crate::save::SaveError;
        if !(1..=crate::save::SAVE_FORMAT_VERSION).contains(&data.format_version) {
            return Err(SaveError::Incompatible(
                "version du format non prise en charge".into(),
            ));
        }
        let identity = story_identity(&self.script);
        let compatibility = match &data.story_identity {
            Some(saved) if saved == &identity => LoadCompatibility::Verified,
            Some(_) => return Err(SaveError::Incompatible("l’histoire a changé depuis cette sauvegarde ; aucune donnée de la partie actuelle n’a été remplacée".into())),
            None => LoadCompatibility::LegacyUnchecked,
        };
        if data.pc > self.script.len()
            || data.call_stack.len() > 128
            || data.call_stack.iter().any(|pc| *pc > self.script.len())
            || data
                .display_random_pc
                .is_some_and(|pc| pc >= self.script.len())
            || data.last_dialogue.as_ref().is_some_and(|snapshot| {
                !matches!(
                    self.script.get(snapshot.pc),
                    Some(Statement::Dialogue { .. })
                )
            })
        {
            return Err(SaveError::Incompatible(
                "position narrative ou pile d’appels invalide".into(),
            ));
        }
        let mut next = data.into_game_state();
        next.accessibility
            .validate()
            .map_err(SaveError::Incompatible)?;
        let mut saved_work = 0usize;
        if next.vars.len() > 4096 {
            return Err(SaveError::Incompatible(
                "plus de 4 096 variables sauvegardées".into(),
            ));
        }
        for value in next.vars.values() {
            saved_work = saved_work
                .checked_add(
                    crate::value_limits::value_work(value)
                        .map_err(|error| SaveError::Incompatible(error.to_string()))?,
                )
                .ok_or_else(|| {
                    SaveError::Incompatible("données de sauvegarde trop grandes".into())
                })?;
            if saved_work > 1_000_000 {
                return Err(SaveError::Incompatible(
                    "données de sauvegarde trop grandes".into(),
                ));
            }
        }
        next.story_identity = Some(identity);
        self.ui_library
            .reconcile_canvas_states(&mut next.ui, &self.functions, &next.vars)
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        let views = self
            .describe_interfaces(&next)
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        self.validate_canvas_renderer(&views)
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        let motions = self
            .describe_motions(&next)
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        let layered = self
            .describe_layered_characters(&next)
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        let epoch = self
            .next_video_epoch()
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        let videos = self
            .describe_videos(&next, epoch)
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        if let Some(waiting) = &next.videos.waiting {
            if !next.videos.tracks[waiting].clip.cinematic
                && !next
                    .pc
                    .checked_sub(1)
                    .and_then(|pc| self.script.get(pc))
                    .is_some_and(|statement| matches!(statement, Statement::VideoWait { .. }))
            {
                return Err(SaveError::Incompatible(
                    "Video wait does not match the saved narrative position".into(),
                ));
            }
        }
        if next.motions.waiting.is_some()
            && !matches!(self.script.get(next.pc), Some(Statement::MotionWait { .. }))
        {
            return Err(SaveError::Incompatible(
                "Animation wait does not match the saved narrative position".into(),
            ));
        }
        for track in next.motions.tracks.values() {
            if !self
                .motion_target_exists(&track.target, &next)
                .map_err(|error| SaveError::Incompatible(error.to_string()))?
            {
                return Err(SaveError::Incompatible(
                    "Saved animation target no longer exists".into(),
                ));
            }
        }
        if !views.is_empty() && !self.renderer.supports_programmable_ui() {
            return Err(SaveError::Incompatible(
                "le moteur de rendu ne prend pas en charge les interfaces de cette sauvegarde"
                    .into(),
            ));
        }
        let previous = std::mem::replace(&mut self.state, next);
        let interaction = match self.current_interaction() {
            Ok(interaction) => interaction,
            Err(error) => {
                self.state = previous;
                return Err(SaveError::Incompatible(error.to_string()));
            }
        };
        self.renderer.restore_screen(&self.state);
        self.video_epoch = epoch;
        if let Err(error) = self
            .renderer
            .update_interfaces(&views)
            .and_then(|_| self.renderer.update_layered_characters(&layered))
            .and_then(|_| self.renderer.update_motions(&motions))
            .and_then(|_| self.renderer.update_videos(&videos))
            .and_then(|_| {
                self.renderer
                    .update_accessibility(&self.state.accessibility)
            })
        {
            self.state = previous;
            self.renderer.restore_screen(&self.state);
            if let Ok(previous_views) = self.interface_views() {
                let _ = self.renderer.update_interfaces(&previous_views);
            }
            let _ = self.refresh_motions();
            let _ = self.refresh_layered_characters();
            if let Ok(views) = self.video_views() {
                let _ = self.renderer.update_videos(&views);
            }
            let _ = self
                .renderer
                .update_accessibility(&self.state.accessibility);
            return Err(SaveError::Incompatible(error));
        }
        if self.renderer.supports_accessibility() {
            let _ = self
                .renderer
                .accessibility_speech(&rvn_ui::accessibility::SpeechRequest::Stop);
        }
        self.history.clear();
        if matches!(
            self.current_interaction(),
            Ok(Some(Interaction::Choice { .. }))
        ) {
            if let Ok(Some(dialogue)) = self.last_dialogue_interaction() {
                self.render_interaction(dialogue);
            }
        }
        if let Some(interaction) = interaction {
            self.render_interaction(interaction);
        }
        self.renderer.loaded_compatibility(compatibility);
        self.interface_epoch = self.interface_epoch.wrapping_add(1);
        Ok(compatibility)
    }

    pub fn is_finished(&self) -> bool {
        self.state.pc >= self.script.len() && self.state.videos.waiting.is_none()
    }
    pub fn get_var(&self, name: &str) -> Option<&Value> {
        self.state.vars.get(name)
    }

    fn describe_layered_characters(
        &self,
        state: &GameState,
    ) -> Result<Vec<crate::composition::LayeredView>, RuntimeError> {
        if !state.layered.characters.is_empty() && !self.renderer.supports_layered_characters() {
            return Err(self.eval_err(
                EvalError::InvalidFunction(
                    "This renderer does not support layered characters".into(),
                ),
                "Renderer capability: compositions",
            ));
        }
        state.layered.views(&state.sprites).map_err(|message| {
            self.eval_err(EvalError::InvalidFunction(message), "Character composition")
        })
    }
    pub fn layered_character_views(
        &self,
    ) -> Result<Vec<crate::composition::LayeredView>, RuntimeError> {
        self.describe_layered_characters(&self.state)
    }
    pub fn refresh_layered_characters(&mut self) -> Result<(), RuntimeError> {
        let views = self.describe_layered_characters(&self.state)?;
        self.renderer
            .update_layered_characters(&views)
            .map_err(|message| {
                self.eval_err(
                    EvalError::InvalidFunction(message),
                    "Renderer capability: compositions",
                )
            })
    }
    fn motion_target_exists(
        &self,
        target: &crate::motion::MotionTarget,
        state: &GameState,
    ) -> Result<bool, RuntimeError> {
        use crate::motion::MotionTarget;
        Ok(match target {
            MotionTarget::Background => !state.background_image.is_empty(),
            MotionTarget::Sprite { id } => {
                state.sprites.get(id).is_some_and(|sprite| sprite.visible)
            }
            MotionTarget::Layer { id, layer } => {
                state.sprites.get(id).is_some_and(|sprite| sprite.visible)
                    && state.layered.layer_visible(id, layer)
            }
            MotionTarget::Interface { screen, element } => self
                .describe_interfaces(state)?
                .iter()
                .find(|view| view.name == *screen)
                .is_some_and(|view| view.root.find(element).is_some()),
        })
    }
    fn cancel_absent_motions(
        &self,
        state: &GameState,
        motions: &mut crate::motion::MotionState,
    ) -> Result<(), RuntimeError> {
        let mut absent = Vec::new();
        for (key, track) in &motions.tracks {
            if !self.motion_target_exists(&track.target, state)? {
                absent.push(key.clone());
            }
        }
        for key in absent {
            motions.tracks.remove(&key);
        }
        Ok(())
    }
    fn describe_motions(
        &self,
        state: &GameState,
    ) -> Result<Vec<crate::motion::MotionView>, RuntimeError> {
        let views = state
            .motions
            .views()
            .map_err(|message| self.eval_err(EvalError::InvalidFunction(message), "Animations"))?;
        if !views.is_empty() && !self.renderer.supports_composable_motion() {
            return Err(self.eval_err(
                EvalError::InvalidFunction(
                    "This renderer does not support composable animations".into(),
                ),
                "Renderer capability: animations",
            ));
        }
        Ok(views)
    }
    pub fn refresh_motions(&mut self) -> Result<(), RuntimeError> {
        let views = self.describe_motions(&self.state)?;
        self.renderer.update_motions(&views).map_err(|message| {
            self.eval_err(
                EvalError::InvalidFunction(message),
                "Renderer capability: animations",
            )
        })
    }
    /// Advance clocks without advancing dialogue or adding rollback frames.
    /// Menu pause is owned by the host: it simply does not tick game time.
    pub fn tick_motions(&mut self, seconds: f64) -> Result<(), RuntimeError> {
        if !seconds.is_finite() || !(0.0..=3600.0).contains(&seconds) {
            return Err(self.eval_err(
                EvalError::InvalidFunction(
                    "Animation delta must be finite and between 0 and 3600 seconds".into(),
                ),
                "Animation clock",
            ));
        }
        if self.state.motions.tracks.is_empty() && self.state.motions.waiting.is_none() {
            return Ok(());
        }
        let mut candidate = self.state.motions.clone();
        self.cancel_absent_motions(&self.state, &mut candidate)?;
        candidate.tick(seconds).map_err(|message| {
            self.eval_err(EvalError::InvalidFunction(message), "Animation clock")
        })?;
        let mut pc = self.state.pc;
        if candidate
            .waiting
            .as_ref()
            .is_some_and(|key| candidate.tracks.get(key).is_none_or(|track| !track.running))
        {
            if !matches!(self.script.get(pc), Some(Statement::MotionWait { .. })) {
                return Err(self.eval_err(
                    EvalError::InvalidFunction(
                        "Animation wait does not match the narrative position".into(),
                    ),
                    "Animation clock",
                ));
            }
            candidate.waiting = None;
            pc += 1;
        }
        let views = candidate
            .views()
            .map_err(|message| self.eval_err(EvalError::InvalidFunction(message), "Animations"))?;
        self.renderer.update_motions(&views).map_err(|message| {
            self.eval_err(
                EvalError::InvalidFunction(message),
                "Renderer capability: animations",
            )
        })?;
        self.state.motions = candidate;
        self.state.pc = pc;
        Ok(())
    }

    pub fn interface_views(&self) -> Result<Vec<rvn_ui::programmable::ScreenView>, RuntimeError> {
        self.describe_interfaces(&self.state)
    }

    fn describe_interfaces(
        &self,
        state: &GameState,
    ) -> Result<Vec<rvn_ui::programmable::ScreenView>, RuntimeError> {
        let mut views = self
            .ui_library
            .views(&state.ui, &self.functions, &state.vars)
            .map_err(|error| self.eval_err(error, "Interface"))?;
        for view in &mut views {
            let mut random = state
                .ui
                .screens
                .iter()
                .find(|screen| screen.name == view.name)
                .unwrap()
                .random;
            let mut problem = None;
            view.root.visit_mut(&mut |component| {
                let mut translate = |key: &Option<String>, text: &mut String| {
                    let Some(key) = key else { return };
                    let translated = self.translate(key);
                    let source = if translated == key {
                        text.as_str()
                    } else {
                        translated
                    };
                    match rvn_parser::parse_interpolated_str(source)
                        .map_err(|error| EvalError::InvalidFunction(error.to_string()))
                        .and_then(|template| {
                            self.functions.interpolate_with_random(
                                &template,
                                &state.vars,
                                &mut random,
                            )
                        }) {
                        Ok(value) if value.len() <= 65_536 => *text = value,
                        Ok(_) => {
                            problem = Some(EvalError::InvalidFunction(
                                "localized interface text is too long".into(),
                            ))
                        }
                        Err(error) => problem = Some(error),
                    }
                };
                translate(&component.text_key, &mut component.text);
                translate(&component.placeholder_key, &mut component.placeholder);
                if component.accessible_label_key.is_some() {
                    translate(
                        &component.accessible_label_key,
                        component.accessible_label.get_or_insert_with(String::new),
                    );
                }
                if !component.option_keys.is_empty() {
                    if component.option_labels.is_empty() {
                        component.option_labels = component.options.clone();
                    }
                    for (key, label) in component
                        .option_keys
                        .iter()
                        .zip(&mut component.option_labels)
                    {
                        translate(&Some(key.clone()), label);
                    }
                }
            });
            if let Some(error) = problem {
                return Err(self.eval_err(error, "Interface translation"));
            }
        }
        Ok(views)
    }

    /// Refresh translated screen text without changing gameplay or RNG.
    pub fn refresh_interfaces(&mut self) -> Result<(), RuntimeError> {
        let views = self.interface_views()?;
        self.validate_canvas_renderer(&views)?;
        self.renderer.update_interfaces(&views).map_err(|error| {
            self.eval_err(
                EvalError::InvalidFunction(error),
                "Renderer capability: interfaces",
            )
        })
    }

    pub fn interface_epoch(&self) -> u64 {
        self.interface_epoch
    }

    fn validate_canvas_renderer(
        &self,
        views: &[rvn_ui::programmable::ScreenView],
    ) -> Result<(), RuntimeError> {
        let mut canvas = false;
        for view in views {
            view.root.visit(&mut |component| {
                canvas |= component.kind == rvn_ui::programmable::ComponentKind::Canvas
            });
        }
        if canvas && !self.renderer.supports_custom_canvas() {
            return Err(self.eval_err(
                EvalError::InvalidFunction(
                    "This renderer does not support programmable canvas drawing".into(),
                ),
                "Renderer capability: custom canvas",
            ));
        }
        Ok(())
    }

    pub fn interface_is_modal(&self) -> bool {
        self.state.ui.screens.iter().any(|screen| screen.modal)
    }

    fn describe_videos(
        &self,
        state: &GameState,
        epoch: u64,
    ) -> Result<Vec<crate::video::VideoView>, RuntimeError> {
        state
            .videos
            .validate()
            .map_err(|message| self.eval_err(EvalError::InvalidFunction(message), "Video state"))?;
        if !state.videos.tracks.is_empty() && !self.renderer.supports_video() {
            return Err(self.eval_err(
                EvalError::InvalidFunction("This renderer does not support video playback".into()),
                "Renderer capability: video",
            ));
        }
        let interfaces = self.describe_interfaces(state)?;
        state
            .videos
            .tracks
            .iter()
            .map(|(id, track)| {
                for handler in [&track.clip.on_end, &track.clip.on_error]
                    .into_iter()
                    .flatten()
                {
                    if !self.ui_library.has_handler(handler) {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction(format!(
                                "Unknown video event handler '{handler}'"
                            )),
                            "Video event",
                        ));
                    }
                }
                if let Some(target) = &track.clip.target {
                    let (screen, element) =
                        target.strip_prefix("ui:").unwrap().split_once('/').unwrap();
                    if !interfaces
                        .iter()
                        .any(|view| view.name == screen && view.root.find(element).is_some())
                    {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction(format!(
                                "Video interface target is absent: {target}"
                            )),
                            "Video target",
                        ));
                    }
                }
                let subtitle = track
                    .clip
                    .subtitle(track.position)
                    .map(|cue| self.translate(&cue.text).to_string());
                Ok(crate::video::VideoView {
                    id: id.clone(),
                    epoch,
                    track: track.clone(),
                    subtitle,
                })
            })
            .collect()
    }
    pub fn video_views(&self) -> Result<Vec<crate::video::VideoView>, RuntimeError> {
        self.describe_videos(&self.state, self.video_epoch)
    }
    /// Host controls use the same transactional path as RVN event handlers.
    pub fn skip_video(&mut self, id: &str) -> Result<(), RuntimeError> {
        let mut next = self.state.clone();
        self.apply_ui_commands(
            &mut next,
            vec![crate::eval::UiCommand::VideoSkip { name: id.into() }],
        )?;
        self.commit_ui_state(next)
    }
    pub fn resume_video(&mut self, id: &str) -> Result<(), RuntimeError> {
        let mut next = self.state.clone();
        self.apply_ui_commands(
            &mut next,
            vec![crate::eval::UiCommand::VideoResume { name: id.into() }],
        )?;
        self.commit_ui_state(next)
    }
    fn next_video_epoch(&self) -> Result<u64, RuntimeError> {
        self.video_epoch
            .checked_add(1)
            .ok_or_else(|| self.eval_err(EvalError::NumericOverflow, "Video playback epoch"))
    }
    fn video_handler(
        &self,
        next: &mut GameState,
        id: &str,
        handler: &str,
    ) -> Result<Vec<crate::eval::UiCommand>, RuntimeError> {
        let track = &next.videos.tracks[id];
        let event = Value::Dict(std::collections::BTreeMap::from([
            ("video".into(), Value::Str(id.into())),
            (
                "kind".into(),
                Value::Str(
                    if track.playback == crate::video::Playback::Ended {
                        "end"
                    } else {
                        "error"
                    }
                    .into(),
                ),
            ),
            ("position".into(), Value::Float(track.position as f32)),
            (
                "message".into(),
                Value::Str(track.message.clone().unwrap_or_default()),
            ),
        ]));
        let (variables, commands) = self
            .ui_library
            .media_event(
                handler,
                event,
                &self.functions,
                &next.vars,
                &mut next.random,
            )
            .map_err(|error| self.eval_err(error, "Video event handler"))?;
        next.vars = variables;
        Ok(commands)
    }
    /// Position reports don't add history frames or request a new seek. Other
    /// feedback is transactional and invalidates old callbacks on completion.
    pub fn video_feedback(
        &mut self,
        epoch: u64,
        id: &str,
        feedback: crate::video::Feedback,
    ) -> Result<bool, RuntimeError> {
        if epoch != self.video_epoch || !self.state.videos.tracks.contains_key(id) {
            return Ok(false);
        }
        let position = matches!(feedback, crate::video::Feedback::Position { .. });
        let mut next = self.state.clone();
        let handler = next.videos.feedback(id, feedback).map_err(|message| {
            self.eval_err(
                EvalError::InvalidFunction(message),
                "Video renderer feedback",
            )
        })?;
        if position {
            self.state.videos = next.videos;
            return Ok(true);
        }
        if let Some(handler) = handler {
            let commands = self.video_handler(&mut next, id, &handler)?;
            self.apply_ui_commands(&mut next, commands)?;
        }
        self.commit_ui_state(next)?;
        Ok(true)
    }

    fn apply_ui_commands(
        &self,
        next: &mut GameState,
        commands: Vec<crate::eval::UiCommand>,
    ) -> Result<(), RuntimeError> {
        self.apply_ui_commands_budgeted(next, commands, &mut crate::ui::CanvasBudget::default())
    }

    fn apply_ui_commands_budgeted(
        &self,
        next: &mut GameState,
        commands: Vec<crate::eval::UiCommand>,
        budget: &mut crate::ui::CanvasBudget,
    ) -> Result<(), RuntimeError> {
        let mut queue: std::collections::VecDeque<_> = commands.into();
        let mut count = 0;
        while let Some(command) = queue.pop_front() {
            count += 1;
            if count > 128 {
                return Err(self.eval_err(
                    EvalError::ExecutionLimit {
                        limit: "128 commandes d’interface et événements de cycle de vie",
                    },
                    "Interface",
                ));
            }
            match &command {
                crate::eval::UiCommand::AccessibilityConfigure { settings } => {
                    if !self.renderer.supports_accessibility() {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction(
                                "This renderer does not support accessibility".into(),
                            ),
                            "Renderer capability: accessibility",
                        ));
                    }
                    next.accessibility = settings.clone();
                    continue;
                }
                crate::eval::UiCommand::AccessibilitySpeak { text } => {
                    if !self.renderer.supports_accessibility() {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction(
                                "This renderer does not support speech synthesis".into(),
                            ),
                            "Renderer capability: accessibility",
                        ));
                    }
                    let spoken = self.translate(text).to_owned();
                    rvn_ui::accessibility::validate_speech(&spoken).map_err(|message| {
                        self.eval_err(EvalError::InvalidFunction(message), "Speech synthesis")
                    })?;
                    next.speech_requests
                        .push(rvn_ui::accessibility::SpeechRequest::Speak(spoken));
                    continue;
                }
                crate::eval::UiCommand::AccessibilityStop => {
                    if !self.renderer.supports_accessibility() {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction(
                                "This renderer does not support speech synthesis".into(),
                            ),
                            "Renderer capability: accessibility",
                        ));
                    }
                    next.speech_requests
                        .push(rvn_ui::accessibility::SpeechRequest::Stop);
                    continue;
                }
                crate::eval::UiCommand::VideoPlay { name, definition } => {
                    if !self.renderer.supports_video() {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction(
                                "This renderer does not support video playback".into(),
                            ),
                            "Renderer capability: video",
                        ));
                    }
                    next.videos
                        .play(name.clone(), definition.clone())
                        .map_err(|message| {
                            self.eval_err(EvalError::InvalidFunction(message), "Video play")
                        })?;
                    if let Some(target) = &definition.target {
                        let (screen, element) =
                            target.strip_prefix("ui:").unwrap().split_once('/').unwrap();
                        if !self
                            .describe_interfaces(next)?
                            .iter()
                            .any(|view| view.name == screen && view.root.find(element).is_some())
                        {
                            return Err(self.eval_err(
                                EvalError::InvalidFunction(format!(
                                    "Video interface target is absent: {target}"
                                )),
                                "Video play",
                            ));
                        }
                    }
                    if definition.cinematic {
                        next.videos.wait(name).map_err(|message| {
                            self.eval_err(EvalError::InvalidFunction(message), "Video cinematic")
                        })?;
                    }
                    continue;
                }
                crate::eval::UiCommand::VideoPause { name } => {
                    next.videos.pause(name).map_err(|message| {
                        self.eval_err(EvalError::InvalidFunction(message), "Video pause")
                    })?;
                    continue;
                }
                crate::eval::UiCommand::VideoResume { name } => {
                    next.videos.resume(name).map_err(|message| {
                        self.eval_err(EvalError::InvalidFunction(message), "Video resume")
                    })?;
                    continue;
                }
                crate::eval::UiCommand::VideoStop { name } => {
                    next.videos.stop(name);
                    continue;
                }
                crate::eval::UiCommand::VideoSeek { name, seconds } => {
                    next.videos.seek(name, *seconds).map_err(|message| {
                        self.eval_err(EvalError::InvalidFunction(message), "Video seek")
                    })?;
                    continue;
                }
                crate::eval::UiCommand::VideoVolume { name, volume } => {
                    next.videos.volume(name, *volume).map_err(|message| {
                        self.eval_err(EvalError::InvalidFunction(message), "Video volume")
                    })?;
                    continue;
                }
                crate::eval::UiCommand::VideoSkip { name } => {
                    if !next
                        .videos
                        .tracks
                        .get(name)
                        .is_some_and(|track| track.clip.skippable)
                    {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction("Skipping this video is not allowed".into()),
                            "Video skip",
                        ));
                    }
                    if let Some(handler) = next
                        .videos
                        .feedback(name, crate::video::Feedback::End)
                        .map_err(|message| {
                        self.eval_err(EvalError::InvalidFunction(message), "Video skip")
                    })? {
                        queue.extend(self.video_handler(next, name, &handler)?);
                    }
                    continue;
                }
                crate::eval::UiCommand::CharacterCompose {
                    character,
                    definition,
                } => {
                    if !self.renderer.supports_layered_characters() {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction(
                                "This renderer does not support layered characters".into(),
                            ),
                            "Renderer capability: compositions",
                        ));
                    }
                    let selected = crate::composition::resolve_attributes(
                        definition,
                        &rvn_ui::composition::Attributes::new(),
                        &definition.defaults,
                        &self.functions,
                        &next.vars,
                        &mut next.random,
                    )
                    .map_err(|error| self.eval_err(error, "Character attribute selector"))?;
                    next.layered
                        .compose(character.clone(), definition.clone())
                        .map_err(|message| {
                            self.eval_err(
                                EvalError::InvalidFunction(message),
                                "Character composition",
                            )
                        })?;
                    next.layered
                        .characters
                        .get_mut(character)
                        .unwrap()
                        .attributes = selected;
                    // A replaced composition has a new authored baseline.
                    next.motions.tracks.retain(|_,track|!matches!(&track.target,crate::motion::MotionTarget::Sprite{id}|crate::motion::MotionTarget::Layer{id,..} if id==character));
                    continue;
                }
                crate::eval::UiCommand::CharacterAttributes {
                    character,
                    attributes,
                } => {
                    let composed = next.layered.characters.get(character).ok_or_else(|| {
                        self.eval_err(
                            EvalError::InvalidFunction(format!(
                                "Character '{character}' has no composition"
                            )),
                            "Character attributes",
                        )
                    })?;
                    let selected = crate::composition::resolve_attributes(
                        &composed.definition,
                        &composed.attributes,
                        attributes,
                        &self.functions,
                        &next.vars,
                        &mut next.random,
                    )
                    .map_err(|error| self.eval_err(error, "Character attribute selector"))?;
                    next.layered
                        .characters
                        .get_mut(character)
                        .unwrap()
                        .attributes = selected;
                    continue;
                }
                crate::eval::UiCommand::MotionPlay { target, definition } => {
                    if !self.motion_target_exists(target, next)? {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction(format!(
                                "Animation target is absent: {}",
                                target.key()
                            )),
                            "Motion play",
                        ));
                    }
                    if definition
                        .validate()
                        .map_err(|message| {
                            self.eval_err(EvalError::InvalidFunction(message), "Motion play")
                        })?
                        .channels
                        & (1 << 12)
                        != 0
                    {
                        if let crate::motion::MotionTarget::Sprite { id } = target {
                            if next.layered.characters.contains_key(id) {
                                return Err(self.eval_err(EvalError::InvalidFunction("Frame animations on a composition must target a named layer, not the entire character".into()),"Motion play"));
                            }
                        }
                        if let crate::motion::MotionTarget::Interface { screen, element } = target {
                            let image = self
                                .describe_interfaces(next)?
                                .iter()
                                .find(|view| view.name == *screen)
                                .and_then(|view| view.root.find(element))
                                .is_some_and(|component| {
                                    component.kind == rvn_ui::programmable::ComponentKind::Image
                                });
                            if !image {
                                return Err(self.eval_err(
                                    EvalError::InvalidFunction(
                                        "Frame animations require an Image interface component"
                                            .into(),
                                    ),
                                    "Motion play",
                                ));
                            }
                        }
                    }
                    next.motions
                        .play(target.clone(), definition.clone())
                        .map_err(|message| {
                            self.eval_err(EvalError::InvalidFunction(message), "Motion play")
                        })?;
                    continue;
                }
                crate::eval::UiCommand::MotionStop { target } => {
                    next.motions.tracks.remove(&target.key());
                    continue;
                }
                _ => {}
            }
            let event = self
                .ui_library
                .command(
                    &mut next.ui,
                    command,
                    &self.functions,
                    &next.vars,
                    &mut next.random,
                )
                .map_err(|error| self.eval_err(error, "Interface"))?;
            if let Some((event, component)) = event {
                queue.extend(
                    self.ui_library
                        .event_budgeted(
                            &mut next.ui,
                            &mut next.vars,
                            &event,
                            &component,
                            &self.functions,
                            &mut next.random,
                            budget,
                        )
                        .map_err(|error| self.eval_err(error, "Interface event"))?,
                );
            }
        }
        Ok(())
    }

    fn commit_ui_state(&mut self, mut next: GameState) -> Result<(), RuntimeError> {
        self.ui_library
            .reconcile_canvas_states(&mut next.ui, &self.functions, &next.vars)
            .map_err(|error| self.eval_err(error, "Canvas instance state"))?;
        let views = self.describe_interfaces(&next)?;
        self.validate_canvas_renderer(&views)?;
        if !views.is_empty() && !self.renderer.supports_programmable_ui() {
            return Err(self.eval_err(
                EvalError::InvalidFunction(
                    "ce moteur de rendu ne prend pas en charge les interfaces programmables".into(),
                ),
                "Renderer capability: interfaces",
            ));
        }
        for view in &views {
            if let Some(screen) = next
                .ui
                .screens
                .iter_mut()
                .find(|screen| screen.name == view.name)
            {
                screen.focus = view.focus.clone();
            }
        }
        let mut next_motions = std::mem::take(&mut next.motions);
        self.cancel_absent_motions(&next, &mut next_motions)?;
        next.motions = next_motions;
        let motions = self.describe_motions(&next)?;
        let layered = self.describe_layered_characters(&next)?;
        // Closing an interface also closes its embedded players and sound.
        next.videos
            .validate()
            .map_err(|error| self.eval_err(EvalError::InvalidFunction(error), "Video state"))?;
        next.videos.tracks.retain(|_, track| {
            track.clip.target.as_ref().is_none_or(|target| {
                let (screen, element) =
                    target.strip_prefix("ui:").unwrap().split_once('/').unwrap();
                views
                    .iter()
                    .any(|view| view.name == screen && view.root.find(element).is_some())
            })
        });
        if next
            .videos
            .waiting
            .as_ref()
            .is_some_and(|id| !next.videos.tracks.contains_key(id))
        {
            next.videos.waiting = None;
        }
        let epoch = if next.videos != self.state.videos {
            self.next_video_epoch()?
        } else {
            self.video_epoch
        };
        let videos = self.describe_videos(&next, epoch)?;
        let previous_views = self.describe_interfaces(&self.state)?;
        let previous_motions = self.describe_motions(&self.state)?;
        let previous_layered = self.describe_layered_characters(&self.state)?;
        let previous_videos = self.video_views()?;
        next.accessibility.validate().map_err(|message| {
            self.eval_err(
                EvalError::InvalidFunction(message),
                "Accessibility settings",
            )
        })?;
        for request in &next.speech_requests {
            self.renderer
                .validate_accessibility_speech(request)
                .map_err(|message| {
                    self.eval_err(EvalError::InvalidFunction(message), "Speech synthesis")
                })?;
        }
        if let Err(problem) = self
            .renderer
            .update_interfaces(&views)
            .and_then(|_| self.renderer.update_layered_characters(&layered))
            .and_then(|_| self.renderer.update_motions(&motions))
            .and_then(|_| self.renderer.update_videos(&videos))
            .and_then(|_| self.renderer.update_accessibility(&next.accessibility))
        {
            let _ = self.renderer.update_interfaces(&previous_views);
            let _ = self.renderer.update_motions(&previous_motions);
            let _ = self.renderer.update_layered_characters(&previous_layered);
            let _ = self.renderer.update_videos(&previous_videos);
            let _ = self
                .renderer
                .update_accessibility(&self.state.accessibility);
            return Err(self.eval_err(
                EvalError::InvalidFunction(problem),
                "Renderer capability: interfaces/animations",
            ));
        }
        // Speech is emitted only after all state and renderer validations pass.
        // No transient request enters a save or a history snapshot.
        for request in std::mem::take(&mut next.speech_requests) {
            if let Err(problem) = self.renderer.accessibility_speech(&request) {
                let _ = self
                    .renderer
                    .accessibility_speech(&rvn_ui::accessibility::SpeechRequest::Stop);
                let _ = self.renderer.update_interfaces(&previous_views);
                let _ = self.renderer.update_layered_characters(&previous_layered);
                let _ = self.renderer.update_motions(&previous_motions);
                let _ = self.renderer.update_videos(&previous_videos);
                let _ = self
                    .renderer
                    .update_accessibility(&self.state.accessibility);
                return Err(self.eval_err(EvalError::InvalidFunction(problem), "Speech synthesis"));
            }
        }
        self.state = next;
        self.video_epoch = epoch;
        Ok(())
    }

    /// A failed event leaves variables, focus, controls and RNG untouched.
    pub fn interface_event(&mut self, event: crate::ui::UiInput) -> Result<(), RuntimeError> {
        use rvn_ui::programmable::ScreenEventKind;
        if event.screen.len() > 128
            || event.element.len() > 128
            || event.key.as_ref().is_some_and(|key| key.len() > 256)
        {
            return Err(self.eval_err(
                EvalError::InvalidFunction("événement d’interface trop volumineux".into()),
                "Interface event",
            ));
        }
        if let Some(value) = &event.value {
            crate::value_limits::value_work(value)
                .map_err(|error| self.eval_err(error, "Interface event"))?;
        }
        if matches!(
            event.kind,
            ScreenEventKind::Open | ScreenEventKind::Close | ScreenEventKind::Tick
        ) {
            return Err(self.eval_err(
                EvalError::InvalidFunction(
                    "événement interne de cycle de vie ou simulation".into(),
                ),
                "Interface event",
            ));
        }
        let views = self.interface_views()?;
        let view = views
            .iter()
            .find(|view| view.name == event.screen)
            .ok_or_else(|| {
                self.eval_err(
                    EvalError::InvalidFunction(format!("écran fermé : {}", event.screen)),
                    "Interface event",
                )
            })?;
        if event.kind != ScreenEventKind::PointerCancel
            && views
                .iter()
                .rev()
                .find(|screen| screen.modal)
                .is_some_and(|modal| (view.layer, view.order) < (modal.layer, modal.order))
        {
            return Err(self.eval_err(
                EvalError::InvalidFunction("interface masquée par un écran modal".into()),
                "Interface event",
            ));
        }
        let component = view.root.find(&event.element).ok_or_else(|| {
            self.eval_err(
                EvalError::InvalidFunction(format!("élément inconnu : {}", event.element)),
                "Interface event",
            )
        })?;
        if event.kind != ScreenEventKind::PointerCancel && !view.root.available(&event.element) {
            return Err(self.eval_err(
                EvalError::InvalidFunction("élément masqué ou désactivé".into()),
                "Interface event",
            ));
        }
        let mut next = self.state.clone();
        let mut budget = crate::ui::CanvasBudget::default();
        let commands = self
            .ui_library
            .event_budgeted(
                &mut next.ui,
                &mut next.vars,
                &event,
                component,
                &self.functions,
                &mut next.random,
                &mut budget,
            )
            .map_err(|error| self.eval_err(error, "Interface event"))?;
        self.apply_ui_commands_budgeted(&mut next, commands, &mut budget)?;
        let previous = self.state.clone();
        self.commit_ui_state(next)?;
        // Focus and unhandled key/click events do not consume rollback steps.
        // A focus handler can still change gameplay: preserve that transition.
        let mut before_ui = previous.ui.clone();
        let mut after_ui = self.state.ui.clone();
        for screen in &mut before_ui.screens {
            screen.focus = None;
        }
        for screen in &mut after_ui.screens {
            screen.focus = None;
        }
        if previous.vars != self.state.vars
            || previous.random != self.state.random
            || before_ui != after_ui
        {
            self.history.push(previous, None);
        }
        Ok(())
    }

    /// The host calls this only during active game simulation, never from a
    /// wall-clock callback while paused, unfocused or in the save/menu screen.
    /// One frame is atomic; it adds no rollback entry of its own.
    pub fn interface_tick(&mut self, seconds: f64) -> Result<(), RuntimeError> {
        use rvn_ui::programmable::{ComponentKind, ScreenEventKind};
        if !seconds.is_finite() || !(0.0..=0.25).contains(&seconds) {
            return Err(self.eval_err(
                EvalError::InvalidFunction(
                    "Canvas delta must be finite and between 0 and 0.25 seconds".into(),
                ),
                "Canvas clock",
            ));
        }
        if seconds == 0.0 || self.state.ui.screens.is_empty() {
            return Ok(());
        }
        let views = self.interface_views()?;
        let modal = views
            .iter()
            .rev()
            .find(|view| view.modal)
            .map(|view| (view.layer, view.order));
        let mut targets = Vec::new();
        for view in &views {
            if modal.is_some_and(|modal| (view.layer, view.order) < modal) {
                continue;
            }
            view.root.visit(&mut |component| {
                if component.kind == ComponentKind::Canvas && view.root.available(&component.id) {
                    targets.push((view.name.clone(), component.id.clone()));
                }
            });
        }
        if targets.is_empty() {
            return Ok(());
        }
        let mut next = self.state.clone();
        self.ui_library
            .reconcile_canvas_states(&mut next.ui, &self.functions, &next.vars)
            .map_err(|error| self.eval_err(error, "Canvas clock"))?;
        let mut budget = crate::ui::CanvasBudget::default();
        for (screen, element) in targets {
            // A previous callback may have removed this component or placed a
            // modal over it. Do not deliver a stale tick to a replacement.
            let views = self.describe_interfaces(&next)?;
            let Some(view) = views.iter().find(|view| view.name == screen) else {
                continue;
            };
            if views
                .iter()
                .rev()
                .find(|view| view.modal)
                .is_some_and(|modal| (view.layer, view.order) < (modal.layer, modal.order))
                || !view.root.available(&element)
            {
                continue;
            }
            let Some(component) = view
                .root
                .find(&element)
                .filter(|component| component.kind == ComponentKind::Canvas)
            else {
                continue;
            };
            let Some(entry) = next
                .ui
                .screens
                .iter_mut()
                .find(|instance| instance.name == screen)
                .and_then(|instance| instance.canvas_states.get_mut(&element))
            else {
                continue;
            };
            entry.elapsed += seconds;
            if !entry.elapsed.is_finite() || entry.elapsed > 1.0e12 {
                return Err(self.eval_err(
                    EvalError::InvalidFunction("Canvas simulation time overflow".into()),
                    "Canvas clock",
                ));
            }
            if !component.events.contains_key(&ScreenEventKind::Tick) {
                continue;
            }
            let value = Value::Dict(std::collections::BTreeMap::from([(
                "dt".into(),
                Value::Float(seconds as f32),
            )]));
            let event = crate::ui::UiInput {
                screen: screen.clone(),
                element,
                kind: ScreenEventKind::Tick,
                value: Some(value),
                key: None,
            };
            let commands = self
                .ui_library
                .event_budgeted(
                    &mut next.ui,
                    &mut next.vars,
                    &event,
                    component,
                    &self.functions,
                    &mut next.random,
                    &mut budget,
                )
                .map_err(|error| self.eval_err(error, "Canvas tick handler"))?;
            self.apply_ui_commands_budgeted(&mut next, commands, &mut budget)?;
        }
        self.commit_ui_state(next)
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
        let parts: Vec<&str> = action.split_whitespace().collect();
        if let [operation, target] = parts.as_slice() {
            match *operation {
                "jump" => {
                    self.state.pc = self.resolve(target)?;
                    self.state.display_random_pc = None;
                    return Ok(());
                }
                "call" => {
                    if self.state.call_stack.len() >= 128 {
                        return Err(self.eval_err(
                            EvalError::ExecutionLimit {
                                limit: "128 appels narratifs imbriqués",
                            },
                            "Timer Call",
                        ));
                    }
                    let idx = self.resolve(target)?;
                    self.state.call_stack.push(self.state.pc + 1);
                    self.state.pc = idx;
                    self.state.display_random_pc = None;
                    return Ok(());
                }
                _ => {}
            }
        }
        Err(self.eval_err(
            EvalError::InvalidFunction(format!(
                "action de minuterie invalide : {action:?} ; utiliser jump <label> ou call <label>"
            )),
            "Timer",
        ))
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
        let mut remaining = crate::eval::MAX_COMPUTATION_STEPS;
        self.step_silent_bounded(&mut remaining)
    }

    fn consume_step(&self, remaining: &mut usize) -> Result<(), RuntimeError> {
        if *remaining == 0 {
            return Err(self.eval_err(
                EvalError::ExecutionLimit {
                    limit: "100 000 instructions sans interaction",
                },
                "step_silent",
            ));
        }
        *remaining -= 1;
        Ok(())
    }

    fn step_silent_bounded(&mut self, remaining: &mut usize) -> Result<(), RuntimeError> {
        loop {
            if self.state.motions.waiting.is_some() || self.state.videos.waiting.is_some() {
                return Ok(());
            }
            if self.is_finished() {
                return Ok(());
            }
            let stmt = self.script[self.state.pc].clone();
            if Self::is_interactive(&stmt) {
                return Ok(());
            }
            self.consume_step(remaining)?;
            self.exec_silent(stmt)?;
        }
    }

    pub fn peek_statement(&self) -> Option<&Statement> {
        self.script.get(self.state.pc)
    }
}
