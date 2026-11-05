use rvn_parser::{Hotspot, Position, Script, Statement, Transition, Value};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::RuntimeError;
use crate::eval::{EvalError, FunctionLibrary};
use crate::locale::LocaleManager;
use crate::renderer::Renderer;
use crate::rollback::{HistoryDisplay, RollbackHistory};
use crate::save::{SaveData, SaveManager};
use crate::types::{CinematicState, GameState, MusicState, SpriteState, TypewriterState};

#[path = "engine_typewriter_consumer.rs"]
mod typewriter_consumer;
#[path="engine_classic_consumer.rs"]
mod classic_consumer;
#[path="engine_game_tasks.rs"]
mod standalone_game_tasks;
#[path="engine_game_sprites.rs"]
mod standalone_game_sprites;
#[path="engine_method_call.rs"]
pub(crate) mod object_methods;
#[path="engine_narrative_native_loops.rs"]
mod narrative_native_loops;

#[path="engine_widget_object_class_1002.rs"]
mod widget_object_class;
#[path="engine_widget_classes_0950.rs"]
mod widget_class_instances;
#[path="engine_enum_startup1025.rs"]
mod enum_startup1025;
#[path="engine_widget_backend_scene_0962.rs"]
mod widget_backend_scene;
#[path="engine_widget_materialization_0963.rs"]
mod widget_materialization;
#[path="engine_widget_load_retirement_0965.rs"]
mod widget_load_retirement;
pub use widget_load_retirement::PreparedWidgetRuntimeRetirement;
#[path="engine_widget_retirement_speech_0975.rs"]
mod widget_retirement_speech;
pub use widget_materialization::WidgetMaterializationOperation;
pub(crate) use widget_materialization::PortableWidgetMaterialization;
#[cfg(test)]
#[path="engine_widget_handler_tests_0956.rs"]
mod widget_handler_tests_0956;
#[path="engine_widget_consumer_0978.rs"]
mod widget_consumer0978;
pub use widget_class_instances::WidgetInstanceHandle;

#[path = "engine_host_random.rs"]
mod host_random;

#[path = "engine_frame.rs"]
mod game_frame;
#[path = "engine_audio.rs"]
mod owned_audio;
#[path="engine_audio_checkpoint.rs"]
mod checkpoint_audio;
#[path="engine_audio_rollback.rs"]
mod audio_rollback;
pub use audio_rollback::{PreparedAudioRollback,AudioRollbackCommitGuard};
#[path = "engine_cg.rs"]
mod owned_cg;
pub use owned_audio::{AudioReportDisposition, AudioReportReceipt};
pub use game_frame::{GameFrameOutcome, GameFrameReports, GameVideoFeedback, GameVideoReport, HostFrameOutcome, HostFrameReports, HostFrameReceiptOutcome, GameFrameReceiptOutcome, VideoReportAuthority, VideoReportDisposition, VideoReportReceipt};

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
fn compiler_consumer_temporaries(original:&[Statement],prepared:&[Statement])->HashMap<usize,String>{
    let mut entries=Vec::new();let mut pending=vec![original];while let Some(block)=pending.pop(){for statement in block{if let Statement::ScheduledConsumer{version:2,scope,consumer,steps,body}=statement{let value=match body.as_slice(){[Statement::ForEach{collection,..}]=>Some(rvn_parser::Expr::Call{name:"__rvn_iterable".into(),args:vec![collection.clone()]}),[Statement::ForLoopWithBreak{first,last,..}]=>Some(rvn_parser::Expr::Call{name:"__rvn_for_bounds".into(),args:vec![first.clone(),last.clone()]}),_=>None};if let Some(value)=value{entries.push((scope,consumer,steps,value));}}pending.extend(crate::consumer_schedule::statement_children(statement));}}
    prepared.iter().enumerate().filter_map(|(pc,statement)|{let Statement::ScheduledConsumer{scope,consumer,steps,body,..}=statement else{return None;};let[Statement::SetVar{name,value}]=body.as_slice()else{return None;};((name.starts_with("__rvn_range_")||name.starts_with("__rvn_for_"))&&entries.iter().any(|(s,c,p,v)|*s==scope&&*c==consumer&&*p==steps&&v==value)).then(||(pc,name.clone()))}).collect()
}

fn flatten_with_returns(
    script: &mut Script,
    extra: &mut Vec<Statement>,
    counter: &mut usize,
    returns: &mut Vec<usize>,
) {
    // A Sequence wrapper gathers once at its genuine header; its authored
    // children remain actual instructions, each with its own consumer gather.
    let mut index=0;while index<script.len(){let mut lanes=None;if let Statement::ScheduledConsumer{version:2,body,..}=&mut script[index]{if let[Statement::Sequence{branches}]=body.as_mut_slice(){lanes=Some(std::mem::take(branches));}}
        if let Some(lanes)=lanes{let children=lanes.into_iter().flatten().collect::<Vec<_>>();script.splice(index+1..index+1,children);}index+=1;}
    let mut index=0;
    while index<script.len(){
        if let Statement::ScheduledConsumer{version:2,scope,consumer,steps,body}=&script[index]{
            if matches!(body.as_slice(),[Statement::ForEach{..}|Statement::ForLoopWithBreak{..}]){
                let(scope,consumer,steps)=(scope.clone(),consumer.clone(),steps.clone());let mut lowered=body.clone();
                flatten_with_returns(&mut lowered,extra,counter,returns);
                let first=lowered.remove(0);lowered.insert(0,Statement::ScheduledConsumer{version:2,scope,consumer,steps,body:vec![first]});
                let count=lowered.len();script.splice(index..=index,lowered);index+=count;continue;
            }
        }index+=1;
    }
    crate::flow_control::lower_legacy(script, counter);
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
            Statement::ScheduledConsumer{version:2,body,..}if matches!(body.as_slice(),[Statement::If{..}|Statement::Choice{..}|Statement::Imagemap{..}|Statement::While{..}])=>{flatten_with_returns(body,extra,counter,returns);},
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
        Expr::LibraryCall{call}=>format!("functions.value_v1({:?}, {:?}, {{{}}})",call.reference().library.as_str(),call.reference().graph_id.to_string(),call.arguments().iter().map(|(id,value)|format!("{:?}:{}",id.as_str(),expr_to_display(value))).collect::<Vec<_>>().join(",")),
        Expr::StandardMacroData {site,output,..} => {
            let method=match output {rvn_parser::scheduled_source::standard_macro_contract::DataOutput::Counter=>"flow.macro_counter_v1",rvn_parser::scheduled_source::standard_macro_contract::DataOutput::IsA=>"flow.macro_is_a_v1"};
            format!("{}({}, {})",method,serde_json::to_string(&site.scope_source()).expect("String serialization"),serde_json::to_string(&site.node_source()).expect("String serialization"))
        }
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

struct InterfaceViewSnapshot {
    locale: Option<LocaleManager>,
    text_resources: crate::string_table::StringTableRevision,
    ui: crate::ui::UiState,
    globals: HashMap<String, Value>,
    views: Vec<rvn_ui::programmable::ScreenView>,
}

impl InterfaceViewSnapshot {
    fn matches(&self, state: &GameState, globals: &HashMap<String, Value>, locale: Option<&LocaleManager>, text_resources: &crate::string_table::StringTableRevision) -> bool {
        // RVN numeric equality intentionally treats signed zero as equal. A
        // pure callback can still observe its text/JSON representation, so a
        // cache key must compare floating-point inputs by their actual bits.
        self.locale.as_ref() == locale
            && &self.text_resources == text_resources
            && self.ui == state.ui
            && self.globals.len() == globals.len()
            && self.globals.iter().all(|(name, value)| {
                globals
                    .get(name)
                    .is_some_and(|other| exact_ui_value(value, other))
            })
            && self
                .ui
                .screens
                .iter()
                .zip(&state.ui.screens)
                .all(|(old, new)| {
                    old.arguments
                        .iter()
                        .zip(&new.arguments)
                        .all(|(a, b)| exact_ui_value(a, b))
                        && old
                            .values
                            .iter()
                            .all(|(id, value)| exact_ui_value(value, &new.values[id]))
                        && old.canvas_states.iter().all(|(id, canvas)| {
                            let other = &new.canvas_states[id];
                            canvas.elapsed.to_bits() == other.elapsed.to_bits()
                                && exact_ui_value(&canvas.state, &other.state)
                        })
                })
    }
}

fn exact_ui_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
        (Value::Text(a), Value::Text(b)) => exact_ui_text(a, b),
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| exact_ui_value(a, b))
        }
        (Value::Dict(a), Value::Dict(b)) => {
            a.len() == b.len()
                && a.iter().all(|(key, value)| {
                    b.get(key).is_some_and(|other| exact_ui_value(value, other))
                })
        }
        _ => left == right,
    }
}

fn exact_ui_text(left: &rvn_parser::localized_text::DeferredText, right: &rvn_parser::localized_text::DeferredText) -> bool {
    use rvn_parser::localized_text::{DeferredText, TextArgument};
    match (left, right) {
        (DeferredText::StringTable { reference: a }, DeferredText::StringTable { reference: b })
        | (DeferredText::NameStringTable { reference: a }, DeferredText::NameStringTable { reference: b }) => a == b,
        (DeferredText::Literal { text: a }, DeferredText::Literal { text: b }) => a == b,
        (DeferredText::Format { pattern: a, arguments: aa }, DeferredText::Format { pattern: b, arguments: ba }) => {
            exact_ui_text(a, b) && aa.len() == ba.len() && aa.iter().all(|(name, value)| {
                ba.get(name).is_some_and(|other| match (value, other) {
                    (TextArgument::Float(a), TextArgument::Float(b)) => a.to_bits() == b.to_bits(),
                    (TextArgument::Text(a), TextArgument::Text(b)) => exact_ui_text(a, b),
                    _ => value == other,
                })
            })
        }
        _ => false,
    }
}

#[path="engine_reveal.rs"]
mod portable_reveal;
#[path="engine_unit_owner1003.rs"]
mod unit_owner;

pub struct Engine<R: Renderer> {
    pub script: Script,
    functions: FunctionLibrary,
    consumer_program: crate::consumer_schedule::ConsumerProgram,
    widget_classes:std::sync::Arc<crate::widget_class_source_0950::AdmittedWidgetClasses>,
    pending_widget_backend:Option<std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewport>>,
    // Only disposable Save15 target candidates retain this real old-parent
    // plan. Live publication never inherits its logical-hydration exemption.
    planning_load_retirement:Option<std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>>,
    retirement_speech:Option<std::sync::Arc<Mutex<widget_retirement_speech::SpeechLog>>>,
    restored_widget_barrier:bool,
    consumer_temporary_writes:HashMap<usize,String>,
    initializing_consumers: bool,
    consumer_init_pc: Option<usize>,
    startup: Option<debugger_commands::startup::StartupState>,
    // Actual frozen bytes retained by the existing Startup owner and handoff.
    frozen_startup_program:Option<std::sync::Arc<rvn_parser::ParsedProgram>>,
    startup_video_trace:Option<std::sync::Arc<Mutex<debugger_commands::startup::video_commands::StartupVideoTrace>>>,
    ui_library: crate::ui::UiLibrary,
    // Pure, untranslated descriptions only. Exact input keys also detect
    // direct mutations of the public game state. Two entries retain both sides
    // of a renderer transaction; no cache or geometry enters saves/history.
    interface_view_cache: Mutex<Vec<InterfaceViewSnapshot>>,
    #[cfg(test)]
    interface_view_builds: std::sync::atomic::AtomicUsize,
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
    /// Producer identity is never saved and cannot restart in another Engine.
    engine_instance_id: u64,
    // Display delivery authorization is never part of SaveData or GameState.
    display_generation: u64,
    display_owner: Option<crate::scene::ActivationId>,
    display_history_entry: Option<u64>,
    // The displayed legacy visit outlives non-display UI rollback entries.
    // This process-local authority is never serialized into the game state.
    legacy_preview_visit: Option<LegacyPreviewVisit>,
    /// Root source statements end before the lowered narrative branch blocks.
    /// An Init from an authored branch is not a global initializer.
    init_scan_end: usize,
    narrative: Option<crate::program::PreparedNarrative>,
    /// Runtime-only autonomous Host random state; not saved or rolled back.
    host_random: host_random::HostRandomState,
    host_random_planning: host_random::HostRandomPlanning,
    /// Revokes task callbacks after a transfer, restore or fresh game.
    runtime_epoch: u64,
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

#[path="engine_function_libraries1023.rs"]
mod function_libraries1023;
pub use function_libraries1023::PreparedFunctionLibraryProgram1023;
#[path="engine_preview.rs"]
mod editor_preview_commands;
use editor_preview_commands::LegacyPreviewVisit;
#[path="engine_debugger.rs"]
mod debugger_commands;
#[path="engine_consumer_imagemap.rs"]
mod consumer_imagemap;
#[path="engine_consumer_choice.rs"]
mod consumer_choice_commands;
#[path="engine_consumer_dialogue.rs"]
mod consumer_dialogue_commands;
pub use debugger_commands::{DebugSession,DebugOperation,DebugOutcome,DebugProgress,DebugStateSource,PreparedSave15,Save15CommitGuard,EngineStartup,StartupOperation,StartupCommitGuard,PreparedStartupStep,StartupStepCommitGuard,StartupPublicationSource,StartupBatchSink,StartupHostEffects,StartupHostStepGuard,CommittedStartupStep,StartupObservationSource,StartupVideoCommand,StartupVideoCommandKind,StartupVideoClockSource,StartupInputBatchSink,StartupImagemapSource,StartupVideoReport,PreparedStartupVideoFeedback,StartupVideoFeedbackGuard,CommittedStartupVideoFeedback,AcceptedStartupVideoReport,StartupVideoReportKind,PreparedStartupVideoContinuation,AcceptedStartupVideoTerminal,MAX_STARTUP_VIDEO_REPORTS,MAX_STARTUP_VIDEO_METADATA_BYTES};
pub(crate) use debugger_commands::startup::PortableStartup;

/// Host display context installed before authored initialization. Its assets
/// and culture remain independent of narrative values and saved game state.
#[derive(Debug, Clone, Default)]
pub struct TextRuntimeContext {
    pub resources: std::sync::Arc<crate::string_table::StringTableResources>,
    pub locale: Option<LocaleManager>,
}
impl TextRuntimeContext {
    pub fn new(resources: crate::string_table::StringTableResources, locale: Option<LocaleManager>) -> Self {
        Self { resources: std::sync::Arc::new(resources), locale }
    }
}

#[path="host_consumer.rs"]
mod host_consumer;
#[path="frame_continuation.rs"]
pub(crate) mod frame_continuation;
pub use frame_continuation::{RestoredFrameOperation,HostLifecycleRestoreContext};

impl<R: Renderer> Engine<R> {
    /// The host loads VN assets independently of saved narrative values.
    pub fn text_resources(&self) -> &crate::string_table::StringTableResources {
        self.functions.text_resources()
    }

    pub fn text_resources_mut(&mut self) -> &mut crate::string_table::StringTableResources {
        self.functions.text_resources_mut()
    }

    pub fn new(script: Script, renderer: R, rollback_depth: usize) -> Result<Self, RuntimeError> {
        Self::new_with_persistent(script, renderer, rollback_depth, HashMap::new())
    }

    /// Supply session progression before the authored initialization runs.
    pub fn new_with_persistent(
        script: Script,
        renderer: R,
        rollback_depth: usize,
        persistent_vars: HashMap<String, Value>,
    ) -> Result<Self, RuntimeError> {
        Self::new_with_persistent_and_text_resources(script, renderer, rollback_depth, persistent_vars, Default::default())
    }

    pub fn new_with_text_resources(script: Script, renderer: R, rollback_depth: usize,
        text_resources: crate::string_table::StringTableResources) -> Result<Self, RuntimeError> {
        Self::new_with_persistent_and_text_resources(script, renderer, rollback_depth, HashMap::new(), text_resources)
    }

    /// Install the host's validated asset snapshot before any init conversion.
    pub fn new_with_persistent_and_text_resources(
        script: Script, renderer: R, rollback_depth: usize,
        persistent_vars: HashMap<String, Value>, text_resources: crate::string_table::StringTableResources,
    ) -> Result<Self, RuntimeError> {
        Self::new_with_persistent_and_text_context(script, renderer, rollback_depth, persistent_vars, TextRuntimeContext::new(text_resources, None))
    }

    /// Locale and assets are available to init exactly once, before conversion.
    pub fn new_with_persistent_and_text_context(
        script: Script, renderer: R, rollback_depth: usize,
        persistent_vars: HashMap<String, Value>, text_context: TextRuntimeContext,
    ) -> Result<Self, RuntimeError> {
        Self::new_with_consumer_source(script,renderer,rollback_depth,persistent_vars,text_context,None,false)
    }

    fn new_with_consumer_source(
        script: Script, renderer: R, rollback_depth: usize,
        persistent_vars: HashMap<String,Value>, text_context: TextRuntimeContext,
        consumer_source: Option<crate::consumer_schedule::ConsumerSourceBinding>, defer_init:bool,
    ) -> Result<Self,RuntimeError> {Self::new_with_consumer_source_in(script,renderer,rollback_depth,persistent_vars,text_context,consumer_source,defer_init,None)}
    fn new_with_consumer_source_in(
        mut script: Script, renderer: R, rollback_depth: usize,
        persistent_vars: HashMap<String,Value>, text_context: TextRuntimeContext,
        consumer_source: Option<crate::consumer_schedule::ConsumerSourceBinding>, defer_init:bool,
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>,
    ) -> Result<Self,RuntimeError> {
        Self::new_with_consumer_source_libraries1023_in(script,renderer,rollback_depth,persistent_vars,text_context,consumer_source,defer_init,startup_input,None)
    }
    fn new_with_consumer_source_libraries1023_in(
        mut script: Script, renderer: R, rollback_depth: usize,
        persistent_vars: HashMap<String,Value>, text_context: TextRuntimeContext,
        consumer_source: Option<crate::consumer_schedule::ConsumerSourceBinding>, defer_init:bool,
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>,
    prepared_libraries1023:Option<FunctionLibrary>) -> Result<Self,RuntimeError> {
        if script
            .iter()
            .any(|statement| matches!(statement, Statement::SceneLabel { .. }))
        {
            let prepared = crate::program::PreparedNarrative::prepare(script).map_err(|error| {
                RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error), 0)
            })?;
            return Self::from_narrative_libraries1023_seeded_mode_in(prepared, renderer, rollback_depth, persistent_vars, text_context, consumer_source,defer_init,startup_input,prepared_libraries1023);
        }
        crate::flow_control::validate(&script, false, 0)
            .map_err(|e| RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(e),0))?;
        let mut extra = Vec::new();
        let authored=script.clone();
        let mut counter = 0;
        let mut returns = Vec::new();
        flatten_with_returns(&mut script, &mut extra, &mut counter, &mut returns);
        let init_scan_end = script.len();

        script.push(Statement::Jump {
            target: "__script_end".to_string(),
        });
        let branch_returns = returns.into_iter().map(|pc| pc + script.len()).collect();
        script.extend(extra);
        script.push(Statement::Label {
            name: "__script_end".to_string(),
        });
        let consumer_temporaries=compiler_consumer_temporaries(&authored,&script);

        Self::from_prepared_script_libraries1023_in(
            script,
            renderer,
            rollback_depth,
            branch_returns,
            init_scan_end,
            None,
            (0, 0, 0),
            persistent_vars,
            text_context,
            consumer_source,
            consumer_temporaries,
            defer_init,
            startup_input,
            prepared_libraries1023,
        )
    }

    /// Keep real file/span provenance supplied by the resolved program loader.
    pub fn from_program(program: rvn_parser::ParsedProgram, renderer: R, rollback_depth: usize) -> Result<Self, RuntimeError> {
        Self::from_program_with_persistent(program, renderer, rollback_depth, HashMap::new())
    }

    pub fn from_program_with_consumer_binding(program: rvn_parser::ParsedProgram,
        binding: crate::consumer_schedule::ConsumerSourceBinding, renderer:R, rollback_depth:usize) -> Result<Self,RuntimeError> {
        let actual = crate::consumer_schedule::ConsumerSourceBinding::from_program(&program)
            .map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        if actual != binding {
            return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Consumer source binding differs from frozen ProgramSource/AST/origins".into())),0));
        }
        Self::from_program_bootstrap_with_consumer_binding(program,renderer,rollback_depth,
            HashMap::new(),TextRuntimeContext::default(),false,Some(binding))
    }

    pub fn from_program_with_persistent(
        program: rvn_parser::ParsedProgram,
        renderer: R,
        rollback_depth: usize,
        persistent_vars: HashMap<String, Value>,
    ) -> Result<Self, RuntimeError> {
        Self::from_program_with_persistent_and_text_resources(program,renderer,rollback_depth,persistent_vars,Default::default())
    }

    pub fn from_program_with_persistent_and_text_resources(
        program: rvn_parser::ParsedProgram, renderer: R, rollback_depth: usize,
        persistent_vars: HashMap<String, Value>, text_resources: crate::string_table::StringTableResources,
    ) -> Result<Self, RuntimeError> {
        Self::from_program_with_persistent_and_text_context(program, renderer, rollback_depth, persistent_vars, TextRuntimeContext::new(text_resources, None))
    }

    pub fn from_program_with_persistent_and_text_context(
        program: rvn_parser::ParsedProgram, renderer: R, rollback_depth: usize,
        persistent_vars: HashMap<String, Value>, text_context: TextRuntimeContext,
    ) -> Result<Self, RuntimeError> {
        Self::from_program_bootstrap(program,renderer,rollback_depth,persistent_vars,text_context,false)
    }

    fn from_program_bootstrap(program:rvn_parser::ParsedProgram,renderer:R,rollback_depth:usize,
        persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,defer_init:bool)->Result<Self,RuntimeError>{
        Self::from_program_bootstrap_with_consumer_binding(program,renderer,rollback_depth,persistent_vars,text_context,defer_init,None)
    }
    // Preserve Root0997's exact source-aware binding path. The additional
    // Startup input proof is independent and can never substitute for it.
    fn from_program_bootstrap_with_consumer_binding(program:rvn_parser::ParsedProgram,renderer:R,rollback_depth:usize,
        persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,defer_init:bool,
        supplied_binding:Option<crate::consumer_schedule::ConsumerSourceBinding>)->Result<Self,RuntimeError>{
        Self::from_program_bootstrap_inputs(program,renderer,rollback_depth,persistent_vars,text_context,defer_init,supplied_binding,None)
    }
    fn from_program_bootstrap_in(program:rvn_parser::ParsedProgram,renderer:R,rollback_depth:usize,
        persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,defer_init:bool,
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>)->Result<Self,RuntimeError>{
        Self::from_program_bootstrap_inputs(program,renderer,rollback_depth,persistent_vars,text_context,defer_init,None,startup_input)
    }
    fn from_program_bootstrap_inputs(program:rvn_parser::ParsedProgram,renderer:R,rollback_depth:usize,
        persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,defer_init:bool,
        supplied_binding:Option<crate::consumer_schedule::ConsumerSourceBinding>,
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>)->Result<Self,RuntimeError>{
        Self::from_program_bootstrap_libraries1023_inputs(program,renderer,rollback_depth,persistent_vars,text_context,defer_init,supplied_binding,startup_input,None)
    }
    fn from_program_bootstrap_libraries1023_inputs(program:rvn_parser::ParsedProgram,renderer:R,rollback_depth:usize,
        persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,defer_init:bool,
        supplied_binding:Option<crate::consumer_schedule::ConsumerSourceBinding>,
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>, prepared_libraries1023:Option<FunctionLibrary>)->Result<Self,RuntimeError>{
        if startup_input.is_some()&&!defer_init{return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Startup input requires its retained pending owner".into())),0));}
        // SourceMap reparses frozen ProgramSource bytes and validates the exact
        // AST/inclusion order/ranges before Init or a producer can execute.
        let scheduled = crate::consumer_schedule::contains_owned_plan_declarations(&program.script) || crate::game_audio::has_native_audio(&program.script);
        let consumer_source = if scheduled || defer_init || supplied_binding.is_some() {
            let actual=crate::consumer_schedule::ConsumerSourceBinding::from_program(&program)
                .map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
            if supplied_binding.as_ref().is_some_and(|binding|binding!=&actual) {
                return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Consumer source binding differs from frozen ProgramSource/AST/origins".into())),0));
            }
            Some(actual)
        } else { None };
        // Validate authored dependency contexts before lowering can detach a
        // child from its original scheduled wrapper. Frozen source permits a
        // real host adapter; it does not waive capture/plan validation.
        // Explicit Native narrative admission needs the real prepared branch
        // metadata. Its authored walk and source validation run below, before
        // any Init/publication, through new_with_prepared_native. Raw Legacy
        // constructors retain the existing gate; no boolean waives validation.
        if program.context==rvn_parser::NarrativeContext::Legacy
            || !rvn_parser::scheduled_source::native_loop_contract::contains_explicit_loop(&program.script) {
            match startup_input.as_ref(){
                Some(input)=>crate::consumer_schedule::ConsumerProgram::new_with_source_and_startup(&program.script,consumer_source.as_ref(),input.clone()),
                None=>crate::consumer_schedule::ConsumerProgram::new_with_source(&program.script,consumer_source.as_ref()),
            }.map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        }
        if !defer_init&&crate::consumer_schedule::init_requires_owned_continuation(&program.script){
            if prepared_libraries1023.is_some(){return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Library resources require the real retained Startup adapter before owned Init".into())),0));}
            FunctionLibrary::reject_unadmitted_unit_owner(&program.script).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
            return Self::prepare_owned_startup(std::sync::Arc::new(program),renderer,rollback_depth,persistent_vars,text_context,None,(0,0,0))?.finish_synchronous();
        }
        if program.context == rvn_parser::NarrativeContext::Legacy {
            // Legacy execution keeps its AST contract. Untrusted/missing source
            // sidecars supply no debugger provenance, rather than a fake range.
            let origins=crate::editor_debugger::validate_program_origins(&program).ok().and_then(|_|crate::editor_debugger::collect_origins(&program.script,Some(&program.origins)).ok());
            let mut engine=Self::new_with_consumer_source_libraries1023_in(program.script,renderer,rollback_depth,persistent_vars,text_context,consumer_source,defer_init,startup_input,prepared_libraries1023)?;
            if let Some(origins)=origins{engine.functions.debug_origins=origins;}return Ok(engine);
        }
        let prepared =
            crate::program::PreparedNarrative::from_parsed(program).map_err(|error| {
                RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error), 0)
            })?;
        Self::from_narrative_libraries1023_seeded_mode_in(prepared, renderer, rollback_depth, persistent_vars, text_context, consumer_source,defer_init,startup_input,prepared_libraries1023)
    }

    /// Boundary for compilers which attach real graph/node provenance to an
    /// already prepared program. Preparation is never repeated on this path.
    pub fn from_narrative(prepared: crate::program::PreparedNarrative, renderer: R, rollback_depth: usize) -> Result<Self, RuntimeError> {
        Self::from_narrative_seeded(prepared, renderer, rollback_depth, HashMap::new(), Default::default(), None)
    }

    pub fn from_narrative_with_text_resources(prepared: crate::program::PreparedNarrative, renderer: R,
        rollback_depth: usize, text_resources: crate::string_table::StringTableResources) -> Result<Self, RuntimeError> {
        Self::from_narrative_seeded(prepared, renderer, rollback_depth, HashMap::new(), TextRuntimeContext::new(text_resources, None), None)
    }

    pub fn from_narrative_with_persistent_and_text_context(
        prepared: crate::program::PreparedNarrative, renderer: R, rollback_depth: usize,
        persistent_vars: HashMap<String, Value>, text_context: TextRuntimeContext,
    ) -> Result<Self, RuntimeError> {
        Self::from_narrative_seeded(prepared, renderer, rollback_depth, persistent_vars, text_context, None)
    }

    fn from_narrative_seeded(
        prepared: crate::program::PreparedNarrative,
        renderer: R,
        rollback_depth: usize,
        persistent_vars: HashMap<String, Value>,
        text_context: TextRuntimeContext,
        consumer_source: Option<crate::consumer_schedule::ConsumerSourceBinding>,
    ) -> Result<Self, RuntimeError> {
        Self::from_narrative_seeded_mode(prepared,renderer,rollback_depth,persistent_vars,text_context,consumer_source,false)
    }
    fn from_narrative_seeded_mode(prepared:crate::program::PreparedNarrative,renderer:R,rollback_depth:usize,
        persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,
        consumer_source:Option<crate::consumer_schedule::ConsumerSourceBinding>,defer_init:bool)->Result<Self,RuntimeError>{
        Self::from_narrative_seeded_mode_in(prepared,renderer,rollback_depth,persistent_vars,text_context,consumer_source,defer_init,None)
    }
    fn from_narrative_seeded_mode_in(prepared:crate::program::PreparedNarrative,renderer:R,rollback_depth:usize,
        persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,
        consumer_source:Option<crate::consumer_schedule::ConsumerSourceBinding>,defer_init:bool,
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>)->Result<Self,RuntimeError>{
        Self::from_narrative_libraries1023_seeded_mode_in(prepared,renderer,rollback_depth,persistent_vars,text_context,consumer_source,defer_init,startup_input,None)
    }
    fn from_narrative_libraries1023_seeded_mode_in(prepared:crate::program::PreparedNarrative,renderer:R,rollback_depth:usize,
        persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,
        consumer_source:Option<crate::consumer_schedule::ConsumerSourceBinding>,defer_init:bool,
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>, prepared_libraries1023:Option<FunctionLibrary>)->Result<Self,RuntimeError>{
        Self::from_prepared_script_libraries1023_in(
            prepared.script.clone(),
            renderer,
            rollback_depth,
            prepared.branch_returns.clone(),
            prepared.init_scan_end,
            Some(prepared),
            (0, 0, 0),
            persistent_vars,
            text_context,
            consumer_source,
            HashMap::new(),
            defer_init,
            startup_input,
            prepared_libraries1023,
        )
    }

    /// Start a clean game from this engine's already lowered script. Passing
    /// `self.script` back through `new` would lower choice calls twice and reuse
    /// internal labels, creating recursive calls instead of the original branch.
    pub fn fresh(&self, renderer: R, rollback_depth: usize) -> Result<Self, RuntimeError> {
        self.consumer_program.validate_emitted(&self.script).map_err(|error|self.eval_err(error,"Fresh scheduled program"))?;
        if self.requires_owned_startup_for_fresh(){return self.prepare_fresh_startup(renderer,rollback_depth)?.finish_synchronous();}
        let generations = if self.narrative.is_some() {
            (
                self.runtime_epoch.checked_add(1).ok_or_else(|| {
                    self.eval_err(EvalError::NumericOverflow, "Runtime generation")
                })?,
                self.interface_epoch.checked_add(1).ok_or_else(|| {
                    self.eval_err(EvalError::NumericOverflow, "Interface generation")
                })?,
                self.next_video_epoch()?,
            )
        } else {
            (0, 0, 0)
        };
        let mut library=self.functions.clone();
        library.debug_probe=None;
        let mut engine = Self::from_prepared_script_libraries1023_in(
            self.script.clone(),
            renderer,
            rollback_depth,
            self.branch_returns.clone(),
            self.init_scan_end,
            self.narrative.clone(),
            generations,
            self.persistent_vars.clone(),
            TextRuntimeContext { resources: self.functions.text_resources.clone(), locale: self.locale.clone() },
            self.consumer_program.source.clone(),
            self.consumer_temporary_writes.clone(),
            true,
            None,
            Some(library),
        )?;
        engine.widget_classes=self.widget_classes.clone();
        engine.run_init_blocks()?;
        if self.narrative.is_none() {
            engine.interface_epoch = self.interface_epoch.wrapping_add(1);
            engine.runtime_epoch = self
                .runtime_epoch
                .checked_add(1)
                .ok_or_else(|| self.eval_err(EvalError::NumericOverflow, "Runtime generation"))?;
        }
        engine.functions.debug_origins=self.functions.debug_origins.clone();
        engine.functions.debug_source_map=self.functions.debug_source_map.clone();
        engine.functions.debug_source_map_installed=self.functions.debug_source_map_installed;
        Ok(engine)
    }

    fn from_prepared_script(
        script: Script,
        renderer: R,
        rollback_depth: usize,
        branch_returns: std::collections::HashSet<usize>,
        init_scan_end: usize,
        narrative: Option<crate::program::PreparedNarrative>,
        generations: (u64, u64, u64),
        persistent_vars: HashMap<String, Value>,
        text_context: TextRuntimeContext,
        consumer_source: Option<crate::consumer_schedule::ConsumerSourceBinding>,
        consumer_temporary_writes:HashMap<usize,String>,
        defer_init:bool,
    ) -> Result<Self, RuntimeError> {
        Self::from_prepared_script_in(script,renderer,rollback_depth,branch_returns,init_scan_end,narrative,generations,persistent_vars,text_context,consumer_source,consumer_temporary_writes,defer_init,None)
    }
    fn from_prepared_script_in(
        script: Script,
        renderer: R,
        rollback_depth: usize,
        branch_returns: std::collections::HashSet<usize>,
        init_scan_end: usize,
        narrative: Option<crate::program::PreparedNarrative>,
        generations: (u64, u64, u64),
        persistent_vars: HashMap<String, Value>,
        text_context: TextRuntimeContext,
        consumer_source: Option<crate::consumer_schedule::ConsumerSourceBinding>,
        consumer_temporary_writes:HashMap<usize,String>,
        defer_init:bool,
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>,
    ) -> Result<Self, RuntimeError> {
        Self::from_prepared_script_libraries1023_in(script,renderer,rollback_depth,branch_returns,init_scan_end,narrative,generations,persistent_vars,text_context,consumer_source,consumer_temporary_writes,defer_init,startup_input,None)
    }
    fn from_prepared_script_libraries1023_in(
        script: Script,
        renderer: R,
        rollback_depth: usize,
        branch_returns: std::collections::HashSet<usize>,
        init_scan_end: usize,
        narrative: Option<crate::program::PreparedNarrative>,
        generations: (u64, u64, u64),
        persistent_vars: HashMap<String, Value>,
        text_context: TextRuntimeContext,
        consumer_source: Option<crate::consumer_schedule::ConsumerSourceBinding>,
        consumer_temporary_writes:HashMap<usize,String>,
        defer_init:bool,
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>,
    prepared_libraries1023:Option<FunctionLibrary>) -> Result<Self, RuntimeError> {
        let retained_unit_source1027=(defer_init||startup_input.is_some())&&FunctionLibrary::contains_unit_source(&script);
        let supplied_library_resources1027=prepared_libraries1023.is_some();
        let label_table = script
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                if let Statement::Label { name } | Statement::SceneLabel { name } = s {
                    Some((name.clone(), i))
                } else {
                    None
                }
            })
            .collect();

        // The public FunctionLibrary guard admits authored lexical source,
        // not detached narrative instructions. Establish the real prepared
        // source contract first; the Legacy route retains its prior ordering.
        let prepared_native_consumer=narrative.as_ref().filter(|prepared|!prepared.native_loops.is_empty())
            .map(|prepared|crate::consumer_schedule::ConsumerProgram::new_with_prepared_native_and_startup(prepared,consumer_source.as_ref(),startup_input.clone()))
            .transpose().map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        let function_source=if prepared_native_consumer.is_some(){
            let authored=narrative.as_ref().unwrap().native_loops.authored().ok_or_else(||
                RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Prepared Native functions lost their real authored source".into())),0))?;
            let authored_functions=authored.iter().filter(|statement|matches!(statement,Statement::Function{..}|Statement::UnitFunction{..}|Statement::FunctionLibrary{..})).collect::<Vec<_>>();
            let compiled_functions=script.iter().filter(|statement|matches!(statement,Statement::Function{..}|Statement::UnitFunction{..}|Statement::FunctionLibrary{..})).collect::<Vec<_>>();
            if authored_functions!=compiled_functions{return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Compiled function declarations differ from the admitted authored source".into())),0));}
            authored
        }else{script.as_slice()};

        let mut functions = match prepared_libraries1023 {
            Some(mut functions)=>{functions.debug_probe=None;functions.validate_source_declarations1023(function_source).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;functions},
            None=>FunctionLibrary::from_script(function_source).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?,
        };
        functions.validate_engine_unit_owner(function_source).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        if retained_unit_source1027{
            if !supplied_library_resources1027{return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Unit calls have no admitted retained Startup/debug/save continuation".into())),0));}
            function_libraries1023::startup_joint1027::validate_retained_library_units1027(&functions,function_source).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        }
        if !defer_init{crate::eval::validate_enum_program_literals1027(function_source,&functions.enum_assets1025)
            .map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;}
        let mut consumer_program = match prepared_native_consumer {
            Some(prepared)=>prepared,
            None=>match startup_input{
                Some(input)=>crate::consumer_schedule::ConsumerProgram::new_with_source_and_startup(&script,consumer_source.as_ref(),input),
                None=>crate::consumer_schedule::ConsumerProgram::new_with_source(&script,consumer_source.as_ref()),
            }.map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?,
        };
        if !defer_init&&crate::consumer_schedule::init_requires_owned_continuation(&script){
            return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Native Init loops require their actual pending Startup owner before any Init publication".into())),0));
        }
        consumer_program.source = consumer_source;
        let initial_consumer_frame = if consumer_program.has_plans() {
            Some(crate::consumer_schedule::BoundConsumerFrame::new(
                consumer_program.identity().map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?,
                crate::consumer_schedule::ConsumerOwner::Game))
        } else { None };
        functions.text_resources = text_context.resources;
        if let Some(program)=&narrative{functions.debug_origins=program.debug_origins.clone();}
        let identity = function_libraries1023::story_identity1023(&script,&functions).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        let ui_library = crate::ui::UiLibrary::from_script(&script).map_err(|error| {
            RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error), 0)
        })?;
        let legacy_instances=narrative.is_none()&&consumer_program.source.as_ref().is_some_and(|source|source.definitions.values().any(|definition|definition.native_plans));
        let engine_instance_id=crate::ui_input::new_engine_lineage().map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        let game_audio=(narrative.is_none()&&crate::game_audio::has_native_audio(&script)).then(Default::default);
        let game_tasks=(narrative.is_none()&&crate::game_tasks::has_native_tasks(&script)).then(Default::default);
        let mut engine = Self {
            script,
            functions,
            consumer_program,
            widget_classes:Default::default(),
            pending_widget_backend:None,planning_load_retirement:None,retirement_speech:None,
            restored_widget_barrier:false,
            consumer_temporary_writes,
            initializing_consumers: false,
            consumer_init_pc: None,
            startup:None,
            frozen_startup_program:None,
            startup_video_trace:None,
            ui_library,
            interface_view_cache: Mutex::new(Vec::new()),
            #[cfg(test)]
            interface_view_builds: std::sync::atomic::AtomicUsize::new(0),
            label_table,
            branch_returns,
            state: GameState {
            game_tasks,
            game_audio,
                consumer_dialogue: None,
                consumer_frame: initial_consumer_frame,
                consumer_choice:None,consumer_imagemap:None,
                scene_execution: narrative.as_ref().map(crate::scene::SceneExecution::new),
                accessibility: Default::default(),
                speech_requests: Vec::new(),
                menu_requests: Vec::new(),
                videos: Default::default(),
                layered: Default::default(),
                motions: Default::default(),
                ui: crate::ui::UiState{instance_registry:legacy_instances.then(Default::default),..Default::default()},
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
            locale: text_context.locale,
            persistent_vars,
            input_state: InputState::default(),
            active_timer: None,
            video_epoch: generations.2,
            interface_epoch: generations.1,
            engine_instance_id,
            display_generation: 0,
            display_owner: None,
            display_history_entry: None,
            legacy_preview_visit: None,
            init_scan_end,
            narrative,
            host_random: Default::default(),
            host_random_planning: Default::default(),
            runtime_epoch: generations.0,
        };

        if !defer_init {engine.run_init_blocks()?;}
        Ok(engine)
    }

    // ── Utilitaires internes ──────────────────────────────────────────────────

    fn run_init_blocks(&mut self) -> Result<(), RuntimeError> {
        let mut init_blocks = Vec::new();
        // `use` expands inline, including a chapter's narrative statements.
        // Collect every top-level Init before gameplay, in resolved source order.
        for (i, stmt) in self.script.iter().take(self.init_scan_end).enumerate() {
            if let Statement::Init { body } = stmt {
                init_blocks.push((i, body.clone()));
            }
        }
        // All scheduled plans were validated from the frozen program before
        // any Init publication. Runtime candidates retain the ordinary owner.
        self.initializing_consumers = true;
        for (idx, body) in init_blocks {
            self.consumer_init_pc = Some(idx);
            let result = self.exec_block(&body);
            if result.is_err() { self.initializing_consumers = false; self.consumer_init_pc = None; }
            result?;
            if self.state.pc == idx {
                self.state.pc = idx + 1;
            }
        }
        self.initializing_consumers = false;
        self.consumer_init_pc = None;
        if self.narrative.is_some() {
            // Global initialization is a bootstrap, not a narrative visit.
            // Its statement count must not skip the first scene declaration.
            self.state.pc = 0;
            self.sync_narrative_position();
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

    fn sync_narrative_position(&mut self) {
        if let (Some(program), Some(execution)) = (&self.narrative, &mut self.state.scene_execution)
        {
            execution.live_position = program.position(self.state.pc);
            self.state.call_stack = execution.derived_call_stack();
        }
    }

    pub fn runtime_epoch(&self) -> u64 {
        self.runtime_epoch
    }

    pub fn scene_resource_token(
        &self,
        kind: crate::scene_resources::ResourceTaskKind,
        alias: &str,
    ) -> Option<crate::scene_resources::ResourceToken> {
        let execution = self.state.scene_execution.as_ref()?;
        let task = execution.resources.task(kind, alias, execution.owner())?;
        execution
            .resources
            .effect_token(self.runtime_epoch, &task.key, execution.owner())
    }

    /// Hosts install neutral effect data before advancing any ECS clocks.
    /// Captures do not republish a renderer batch and cannot recurse into it.
    pub fn register_scene_effect(
        &mut self,
        key: crate::scene_tasks::TaskKey,
        target: crate::scene_resources::TargetIdentity,
        effect: crate::scene_resources::SavedEffect,
    ) -> Result<crate::scene_resources::ResourceToken, RuntimeError> {
        crate::scene_resources::register_effect(
            &mut self.state,
            self.runtime_epoch,
            key,
            target,
            effect,
        )
        .map_err(|message| self.scene_resource_error(message))
    }

    pub fn capture_scene_effect(
        &mut self,
        token: &crate::scene_resources::ResourceToken,
        effect: crate::scene_resources::SavedEffect,
    ) -> Result<bool, RuntimeError> {
        let Some(registry) = self.state.effect_resource_registry(token.target.kind) else {
            return Ok(false);
        };
        if !registry.accepts(token,self.runtime_epoch,self.state.audio_active_owner())
        {
            return Ok(false);
        }
        let mut next = self.state.clone();
        let owner=next.audio_active_owner();
        let registry=next.effect_resource_registry_mut(token.target.kind).unwrap();
        let changed = registry
            .capture_effect(token, self.runtime_epoch, owner, effect)
            .map_err(|message| self.scene_resource_error(message))?;
        crate::scene_resources::validate_current_resource_state(&next)
            .map_err(|message| self.scene_resource_error(message))?;
        if changed {
            self.state = next;
        }
        Ok(changed)
    }

    /// Capture only the neutral projection base. Authored parameters and the
    /// logical clock remain controlled by Core, including after a scene call.
    pub fn capture_scene_effect_base(
        &mut self,
        token: &crate::scene_resources::ResourceToken,
        base: crate::scene_resources::EffectProjectionBase,
    ) -> Result<bool, RuntimeError> {
        let Some(registry) = self.state.effect_resource_registry(token.target.kind) else {
            return Ok(false);
        };
        if !registry.accepts(token,self.runtime_epoch,self.state.audio_active_owner())
        {
            return Ok(false);
        }
        let mut next = self.state.clone();
        let owner=next.audio_active_owner();
        let registry=next.effect_resource_registry_mut(token.target.kind).unwrap();
        let changed = registry
            .capture_effect_base(token, self.runtime_epoch, owner, base)
            .map_err(|message| self.scene_resource_error(message))?;
        crate::scene_resources::validate_current_resource_state(&next)
            .map_err(|message| self.scene_resource_error(message))?;
        if changed {
            self.state = next;
        }
        Ok(changed)
    }

    pub fn complete_scene_effect(
        &mut self,
        token: &crate::scene_resources::ResourceToken,
    ) -> Result<bool, RuntimeError> {
        let Some(registry) = self.state.effect_resource_registry(token.target.kind) else {
            return Ok(false);
        };
        if !registry.accepts(token,self.runtime_epoch,self.state.audio_active_owner())
        {
            return Ok(false);
        }
        let mut next = self.state.clone();
        let owner=next.audio_active_owner();
        let registry=next.effect_resource_registry_mut(token.target.kind).unwrap();
        let changed = registry
            .complete_effect(token, self.runtime_epoch, owner);
        crate::scene_resources::validate_current_resource_state(&next)
            .map_err(|message| self.scene_resource_error(message))?;
        if changed {
            self.state = next;
        }
        Ok(changed)
    }

    /// All hosts use this entry API. Editor tab activation is unrelated.
    pub fn enter_label(&mut self, target: &str) -> Result<(), RuntimeError> {
        let pc = self.resolve(target)?;
        if self.narrative.is_none() {
            self.state.pc = pc;
            self.state.display_random_pc = None;
            return Ok(());
        }
        self.transfer_narrative(pc, None, false)
    }

    fn transfer_narrative(
        &mut self,
        target: usize,
        continuation: Option<usize>,
        synthetic: bool,
    ) -> Result<(), RuntimeError> {
        self.transfer_narrative_from(self.state.clone(), target, continuation, synthetic)
    }

    fn transfer_narrative_from(
        &mut self,
        mut next: GameState,
        target: usize,
        continuation: Option<usize>,
        synthetic: bool,
    ) -> Result<(), RuntimeError> {
        next = self.plan_narrative_transfer(next, target, continuation, synthetic)?;
        self.commit_scene_state(next)
    }

    fn plan_narrative_transfer(
        &self,
        mut next: GameState,
        target: usize,
        continuation: Option<usize>,
        synthetic: bool,
    ) -> Result<GameState, RuntimeError> {
        let program = self.narrative.as_ref().expect("opt-in narrative transfer");
        if let Some(scene) = program.scene_entries.get(&target) {
            if continuation.is_some() {
                crate::scene_resources::park_current(&mut next)
            } else {
                crate::scene_resources::abandon_current(&mut next)
            }
            .map_err(|message| self.scene_resource_error(message))?;
            let execution = next.scene_execution.as_mut().ok_or_else(|| {
                self.scene_resource_error("The explicit narrative activation is missing".into())
            })?;
            if let Some(continuation) = continuation {
                execution.call_scene(scene, target, program.position(continuation))
            } else {
                execution.enter(scene, target)
            }
            .map_err(|error| self.eval_err(error, "Scene entry"))?;
            next.pc = target + 1;
        } else {
            if let Some(continuation) = continuation {
                next.scene_execution
                    .as_mut()
                    .ok_or_else(|| {
                        self.scene_resource_error(
                            "The explicit narrative activation is missing".into(),
                        )
                    })?
                    .call_helper(program.position(continuation), synthetic)
                    .map_err(|error| self.eval_err(error, "Narrative call"))?;
            }
            next.pc = target;
        }
        next.display_random_pc = None;
        Ok(next)
    }

    fn narrative_return(&mut self, synthetic: bool) -> Result<(), RuntimeError> {self.narrative_return_from(self.state.clone(),synthetic)}
    fn narrative_return_from(&mut self, mut next:GameState, synthetic: bool) -> Result<(), RuntimeError> {
        let scene_return = next
            .scene_execution
            .as_ref()
            .unwrap()
            .peek_return_kind(synthetic)
            == Some(crate::scene::FrameKind::SceneCall);
        if scene_return {
            crate::scene_resources::abandon_current(&mut next)
                .map_err(|message| self.scene_resource_error(message))?;
        }
        let position = next
            .scene_execution
            .as_mut()
            .expect("narrative activation")
            .return_to_caller(synthetic)
            .map_err(|error| self.eval_err(error, "Narrative return"))?
            .ok_or_else(|| {
                RuntimeError::no_stmt(
                    crate::error::RuntimeErrorKind::ReturnWithoutCall,
                    self.state.pc,
                )
            })?;
        self.narrative
            .as_ref()
            .unwrap()
            .validate_position(&position)
            .map_err(|message| {
                self.eval_err(EvalError::InvalidFunction(message), "Narrative return")
            })?;
        next.pc = position.pc;
        self.finish_native_narrative_body_return(&mut next,&position,synthetic)?;
        next.display_random_pc = None;
        if scene_return {
            crate::scene_resources::resume_current(&mut next)
                .map_err(|message| self.scene_resource_error(message))?;
        }
        self.commit_scene_state(next)
    }

    fn scene_resource_error(&self, message: String) -> RuntimeError {
        self.eval_err(
            EvalError::InvalidFunction(message),
            "Narrative resource ownership",
        )
    }

    /// Build and preflight everything before publishing any authoritative state.
    fn commit_scene_state(&mut self, mut next: GameState) -> Result<(), RuntimeError> {
        self.consumer_program.validate_state_binding(&next).map_err(|error|self.eval_err(error,"Scene consumer ownership"))?;
        let program = self.narrative.as_ref().expect("scene commit program");
        let execution = next
            .scene_execution
            .as_mut()
            .expect("scene commit activation");
        execution.live_position = program.position(next.pc);
        next.call_stack = execution.derived_call_stack();
        self.validate_narrative_loop_gathers(&next,self.locale.as_ref())?;
        // A transfer can reveal a bound tree changed by the child. Reconcile
        // instance state before allocating its exact live projection tasks.
        let structure = self.describe_interface_structure(&next)?;
        self.ui_library.with_locale(self.locale.as_ref())
            .reconcile_canvas_states_from_views(&mut next.ui, &structure)
            .map_err(|error| self.eval_err(error, "Scene interface instance state"))?;
        for view in &structure {
            if let Some(screen) = next
                .ui
                .screens
                .iter_mut()
                .find(|screen| screen.name == view.name && screen.order == view.order)
            {
                screen.focus = view.focus.clone();
            }
        }
        crate::scene_resources::sync_current(&mut next)
            .map_err(|message| self.scene_resource_error(message))?;
        if self.normalize_resident_projection_targets(&mut next)? {
            crate::scene_resources::sync_current(&mut next)
                .map_err(|message| self.scene_resource_error(message))?;
        }
        crate::scene_resources::validate_current_resource_state(&next)
            .map_err(|message| self.scene_resource_error(message))?;
        let execution = next.scene_execution.as_ref().unwrap();
        execution
            .validate(program, &next.vars, &next.call_stack)
            .map_err(|message| self.eval_err(EvalError::InvalidFunction(message), "Scene state"))?;
        let lifecycle_changed = self
            .state
            .scene_execution
            .as_ref()
            .and_then(|execution| execution.owner())
            != execution.owner();
        // Activation/revision checks revoke scene work. Game-owned tasks keep
        // their epoch through ordinary scene calls; restore revokes all work.
        let epoch = self.runtime_epoch;
        let interface_epoch = if lifecycle_changed {
            self.interface_epoch
                .checked_add(1)
                .ok_or_else(|| self.eval_err(EvalError::NumericOverflow, "Interface generation"))?
        } else {
            self.interface_epoch
        };
        let video_epoch = if lifecycle_changed {
            self.next_video_epoch()?
        } else {
            self.video_epoch
        };
        let batch = self.build_scene_batch(next, epoch, interface_epoch, video_epoch)?;
        self.commit_built_scene_batch(batch).map_err(|message|self.eval_err(EvalError::InvalidFunction(message),"Scene renderer preflight"))
    }
    fn commit_built_scene_batch(&mut self,batch:crate::renderer::SceneRenderBatch)->Result<(),String>{
        let prepared=self.pending_widget_backend.take();
        let(batch,owners)=self.prepare_widget_scene_publication(batch,prepared.as_deref()).map_err(|error|error.to_string())?;
        let state=&mut self.state;let runtime=&mut self.runtime_epoch;let interface=&mut self.interface_epoch;let video=&mut self.video_epoch;
        let mut commit=|batch:&crate::renderer::SceneRenderBatch|{
            *state=batch.state.clone();state.menu_requests.clear();state.speech_requests.clear();
            *runtime=batch.runtime_epoch;*interface=batch.interface_epoch;*video=batch.video_epoch;
        };
        if let Some(owners)=owners{self.renderer.commit_widget_scene_batch_atomic(batch,owners,&mut commit)}
        else{self.renderer.commit_scene_batch_atomic(batch,prepared.as_deref(),&mut commit)}
    }

    fn build_scene_batch(
        &self,
        state: GameState,
        runtime_epoch: u64,
        interface_epoch: u64,
        video_epoch: u64,
    ) -> Result<crate::renderer::SceneRenderBatch, RuntimeError> {
        self.build_scene_batch_with_timer_context(state, runtime_epoch, interface_epoch, video_epoch, self.active_timer.as_ref())
    }

    // The standalone Timer transaction validates its prospective compatibility
    // slot together with its prospective GameState. Existing callers keep the
    // actual current slot through build_scene_batch above.
    fn build_scene_batch_with_timer_context(
        &self,
        mut state: GameState,
        runtime_epoch: u64,
        interface_epoch: u64,
        video_epoch: u64,
        timer_context: Option<&(f32, String, f32)>,
    ) -> Result<crate::renderer::SceneRenderBatch, RuntimeError> {
        self.widget_classes.validate_state(&state).map_err(|s|self.scene_resource_error(s))?;
        if runtime_epoch!=self.runtime_epoch {
            let admission=crate::widget_reference_values_0974::WidgetValueAdmission::new(&state,self.widget_classes.clone(),self.instance_id(),runtime_epoch).map_err(|error|self.scene_resource_error(error))?;
            crate::widget_reference_values_0974::visit_state_values_mut(&mut state,&mut |value|admission.value(value,crate::widget_reference_values_0974::ReferencePhase::RebaseHistory)).map_err(|error|self.scene_resource_error(error))?;
        }
        self.validate_standalone_timer_target_with_context(&state, timer_context)?;

        for request in &state.speech_requests {
            self.renderer
                .validate_accessibility_speech(request)
                .map_err(|message| {
                    self.eval_err(
                        EvalError::InvalidFunction(message),
                        "Scene speech preflight",
                    )
                })?;
        }
        for effect in &state.menu_requests {
            self.renderer
                .validate_menu_request(effect, &state)
                .map_err(|message| {
                    self.eval_err(
                        EvalError::InvalidMenuRequest(message),
                        "Scene menu preflight",
                    )
                })?;
        }
        state.accessibility.validate().map_err(|message| {
            self.eval_err(EvalError::InvalidFunction(message), "Scene accessibility")
        })?;
        let globals=self.global_vars_for_state(&state);
        self.ui_library.prepare_owner_properties(&mut state.ui,&self.functions,&globals,self.locale.as_ref()).map_err(|e|self.eval_err(e,"UserWidget owner preflight"))?;
        let interfaces = self.describe_interfaces(&state)?;
        crate::ui::published_owner_hosts(&state.ui,&interfaces).map_err(|s|self.scene_resource_error(s))?;
        self.validate_canvas_renderer(&interfaces)?;
        let motions = self.describe_motions(&state)?;
        let layered = self.describe_layered_characters(&state)?;
        let videos = self.describe_videos(&state, video_epoch, runtime_epoch)?;
        let display = state
            .scene_execution
            .as_ref()
            .and_then(|execution| execution.display.as_ref())
            .map(|checkpoint| self.checkpoint_interaction(checkpoint))
            .transpose()?;
        let mut effects=state.audio_resource_registry().map(|registry|registry.active_effects(state.audio_active_owner()).into_iter().cloned().collect::<Vec<_>>()).unwrap_or_default();
        if let Some(game)=&state.game_tasks{effects.extend(game.resources.active_effects(None).into_iter().cloned());}
        Ok(crate::renderer::SceneRenderBatch {
            previous: self.state.clone(),
            history_previous: self.historical_menu_snapshot(&state)?,
            menu_requests: state.menu_requests.clone(),
            speech_requests: state.speech_requests.clone(),
            retired_widget_speech:Vec::new(),
            state,
            display,
            interfaces,
            motions,
            layered,
            videos,
            effects,
            runtime_epoch,
            interface_epoch,
            video_epoch,
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

    fn narrative_transition_effect(
        transition: &Transition,
        incoming: bool,
        audio: bool,
        layered: bool,
        volume: f32,
    ) -> Result<Option<crate::scene_resources::SavedEffect>, String> {
        use crate::scene_resources::{
            EffectClock, EffectParameters, SavedEffect, VisualTransition,
        };
        let milliseconds = transition.duration_ms();
        if milliseconds == 0 {
            return Ok(None);
        }
        let parameters = if audio {
            EffectParameters::AudioFade {
                // Current audio is an absolute gain. Retired audio is a
                // factor applied once to the already frozen absolute gain.
                from: if incoming { 0.0 } else { 1.0 },
                to: if incoming { volume } else { 0.0 },
                despawn_on_done: !incoming,
            }
        } else {
            let visual = match transition {
                Transition::Fade { .. } => VisualTransition::Fade,
                Transition::Dissolve { .. } => VisualTransition::Dissolve,
                Transition::SlideLeft { .. } => VisualTransition::SlideLeft,
                Transition::SlideRight { .. } => VisualTransition::SlideRight,
                Transition::SlideUp { .. } => VisualTransition::SlideUp,
                Transition::SlideDown { .. } => VisualTransition::SlideDown,
                Transition::ZoomIn { .. } => VisualTransition::ZoomIn,
                Transition::ZoomOut { .. } => VisualTransition::ZoomOut,
                Transition::Wipe { .. } => VisualTransition::Wipe,
                Transition::Blur { .. } => VisualTransition::Blur,
                Transition::None => return Ok(None),
            };
            EffectParameters::VisualFade {
                from: if incoming { 0.0 } else { 1.0 },
                to: if incoming { 1.0 } else { 0.0 },
                despawn_on_finish: !incoming,
                transition: visual,
                base: Default::default(),
                layered,
            }
        };
        let effect = SavedEffect {
            presentation: None,
            base_resolved: false,
            clock: EffectClock {
                duration: f64::from(milliseconds) / 1000.0,
                elapsed: 0.0,
                looping: false,
            },
            parameters,
        };
        effect.validate()?;
        Ok(Some(effect))
    }

    fn plan_retired_transition(
        &self,
        next: &mut GameState,
        source: Option<crate::retired_presentations::RetiredPresentation>,
        transition: &Transition,
    ) -> Result<(), RuntimeError> {
        use crate::retired_presentations::RetiredModel;
        let Some(source) = source else {
            return Ok(());
        };
        let audio = matches!(&source.model, RetiredModel::Music { .. });
        let layered = matches!(
            &source.model,
            RetiredModel::Sprite {
                composition: Some(_),
                ..
            }
        );
        let Some(effect) =
            Self::narrative_transition_effect(transition, false, audio, layered, 1.0)
                .map_err(|message| self.scene_resource_error(message))?
        else {
            return Ok(());
        };
        let owner = Self::command_owner(next);
        crate::scene_resources::register_retired_effect(
            next,
            self.runtime_epoch,
            owner,
            source,
            effect,
        )
        .map_err(|message| self.scene_resource_error(message))?;
        Ok(())
    }

    fn plan_incoming_transition(
        &self,
        next: &mut GameState,
        slot: &crate::retired_presentations::PresentationSlot,
        transition: &Transition,
    ) -> Result<(), RuntimeError> {
        use crate::retired_presentations::PresentationSlot;
        use crate::scene_resources::{register_effect, resource_key, ResourceTaskKind};
        use crate::scene_tasks::TaskOwner;
        let (kind, alias) = slot.source_address();
        let target = next
            .audio_resource_registry().ok_or_else(||self.scene_resource_error("Missing Current registry".into()))?
            .target(&TaskOwner::Game, kind, alias)
            .cloned()
            .ok_or_else(|| {
                self.scene_resource_error("Incoming model has no exact Current target".into())
            })?;
        let audio = matches!(slot, PresentationSlot::Music);
        let layered = match slot {
            PresentationSlot::Sprite { id } => next.layered.characters.contains_key(id),
            _ => false,
        };
        let Some(effect) =
            Self::narrative_transition_effect(transition, true, audio, layered, next.music.volume)
                .map_err(|message| self.scene_resource_error(message))?
        else {
            return Ok(());
        };
        let effect_alias = match slot {
            PresentationSlot::Background => "background/fade".to_string(),
            PresentationSlot::Sprite { id } => format!("sprite/{id}/fade"),
            PresentationSlot::Music => "music/fade".to_string(),
            PresentationSlot::Cinematic => "cinematic/fade".to_string(),
        };
        let owner = Self::command_owner(next);
        let key = resource_key(owner, ResourceTaskKind::Effect, &effect_alias);
        register_effect(next, self.runtime_epoch, key, target, effect)
            .map_err(|message| self.scene_resource_error(message))?;
        Ok(())
    }

    fn exec_narrative_background(
        &mut self,
        background: String,
        transition: Transition,
    ) -> Result<(), RuntimeError> {
        use crate::retired_presentations::{snapshot_retired_source, PresentationSlot};
        use crate::scene_resources::{CancellationReason, TargetKind};
        use crate::scene_tasks::TaskOwner;
        let slot = PresentationSlot::Background;
        let source = snapshot_retired_source(&self.state, &slot)
            .map_err(|message| self.scene_resource_error(message))?;
        let mut next = self.state.clone();
        crate::scene_resources::cancel_retired_story_slot(
            &mut next,
            &slot,
            CancellationReason::TargetReplaced,
        )
        .map_err(|message| self.scene_resource_error(message))?;
        if background.is_empty() {
            next.background_image.clear();
            crate::scene_resources::remove_target(
                &mut next,
                &TaskOwner::Game,
                TargetKind::Background,
                "background",
            )
            .map_err(|message| self.scene_resource_error(message))?;
        } else {
            crate::scene_resources::replace_target(
                &mut next,
                TaskOwner::Game,
                TargetKind::Background,
                "background",
            )
            .map_err(|message| self.scene_resource_error(message))?;
            next.background_image = background;
        }
        next.last_transition = transition.clone();
        self.plan_retired_transition(&mut next, source, &transition)?;
        if !next.background_image.is_empty() {
            self.plan_incoming_transition(&mut next, &slot, &transition)?;
        }
        next.pc += 1;
        self.commit_ui_state(next)
    }

    fn exec_narrative_music_play(
        &mut self,
        file: String,
        transition: Transition,
    ) -> Result<(), RuntimeError> {
        self.exec_narrative_music_play_from(self.state.clone(),file,transition)
    }
    fn exec_narrative_music_play_from(&mut self,mut next:GameState,file:String,transition:Transition)->Result<(),RuntimeError>{
        use crate::retired_presentations::{snapshot_retired_source, PresentationSlot};
        use crate::scene_resources::{CancellationReason, TargetKind};
        use crate::scene_tasks::TaskOwner;
        let slot = PresentationSlot::Music;
        let source = snapshot_retired_source(&next, &slot)
            .map_err(|message| self.scene_resource_error(message))?;
        crate::scene_resources::cancel_retired_story_slot(
            &mut next,
            &slot,
            CancellationReason::TargetReplaced,
        )
        .map_err(|message| self.scene_resource_error(message))?;
        crate::scene_resources::replace_target(
            &mut next,
            TaskOwner::Game,
            TargetKind::Music,
            "music",
        )
        .map_err(|message| self.scene_resource_error(message))?;
        next.music.current_file = Some(file);
        next.last_transition = transition.clone();
        self.plan_retired_transition(&mut next, source, &transition)?;
        self.plan_incoming_transition(&mut next, &slot, &transition)?;
        next.pc += 1;
        self.commit_audio_state(next)
    }

    fn exec_narrative_music_stop(&mut self, transition: Transition) -> Result<(), RuntimeError> {
        self.exec_narrative_music_stop_from(self.state.clone(),transition)
    }
    fn exec_narrative_music_stop_from(&mut self,mut next:GameState,transition:Transition)->Result<(),RuntimeError>{
        use crate::retired_presentations::{snapshot_retired_source, PresentationSlot};
        use crate::scene_resources::{CancellationReason, TargetKind};
        use crate::scene_tasks::TaskOwner;
        let slot = PresentationSlot::Music;
        let source = snapshot_retired_source(&next, &slot)
            .map_err(|message| self.scene_resource_error(message))?;
        // An absent Current slot is an idempotent Stop. Its older outgoing
        // clock belongs to its owner and must not be restarted or removed.
        if source.is_some() {
            crate::scene_resources::cancel_retired_story_slot(
                &mut next,
                &slot,
                CancellationReason::TargetRemoved,
            )
            .map_err(|message| self.scene_resource_error(message))?;
            next.music.current_file = None;
            crate::scene_resources::remove_target(
                &mut next,
                &TaskOwner::Game,
                TargetKind::Music,
                "music",
            )
            .map_err(|message| self.scene_resource_error(message))?;
            next.last_transition = transition.clone();
            self.plan_retired_transition(&mut next, source, &transition)?;
        }
        next.pc += 1;
        self.commit_audio_state(next)
    }

    fn exec_narrative_music_volume_from(&mut self,mut next:GameState,level:f32)->Result<(),RuntimeError>{
                    // Current fades use absolute gain. A logical volume write
                    // therefore supersedes this exact Current channel, even
                    // when its Scene owner is parked. Historical Retired
                    // presentations have independent targets and frozen gain.
                    let current = next.audio_resource_registry().and_then(|registry| {
                        registry.target(
                            &crate::scene_tasks::TaskOwner::Game,
                            crate::scene_resources::TargetKind::Music,
                            "music",
                        ).cloned()
                    });
                    if let Some(current) = current {
                        crate::scene_resources::supersede_effect_channel(
                            &mut next,
                            &current,
                            crate::scene_resources::EffectChannel::AudioFade,
                        ).map_err(|message| self.scene_resource_error(message))?;
                    } else if next.music.current_file.is_some() {
                        return Err(self.scene_resource_error(
                            "Current music has no exact Game incarnation".into(),
                        ));
                    }
                    next.music.volume = level;
                    next.pc += 1;
                    self.commit_audio_state(next)
    }

    fn exec_show(
        &mut self,
        id: &str,
        emotion: Option<String>,
        position: Option<Position>,
        transition: Transition,
    ) -> Result<(), RuntimeError> {
        let slot = crate::retired_presentations::PresentationSlot::Sprite { id: id.into() };
        let source = if self.narrative.is_some() {
            crate::retired_presentations::snapshot_retired_source(&self.state, &slot)
                .map_err(|message| self.scene_resource_error(message))?
        } else {
            None
        };
        let resolved = position.unwrap_or_else(|| {
            self.state
                .sprites
                .get(id)
                .map(|s| s.position.clone())
                .unwrap_or(Position::Center)
        });
        let from = self.state.sprites.get(id).cloned();
        let mut next = self.state.clone();
        if self.narrative.is_some() {
            crate::scene_resources::cancel_retired_story_slot(
                &mut next,
                &slot,
                crate::scene_resources::CancellationReason::TargetReplaced,
            )
            .map_err(|message| self.scene_resource_error(message))?;
        }
        next.last_transition = transition.clone();
        next.sprites.insert(
            id.to_string(),
            SpriteState::new(emotion.clone(), resolved.clone()),
        );
        if let Some(game)=&mut next.game_tasks{
            game.sprite_styles.remove(id);game.legacy_sprite_effects.remove(id);
            if let Some(target)=game.resources.target(&crate::scene_tasks::TaskOwner::Game,crate::scene_resources::TargetKind::Sprite,id).cloned(){
                let keys=game.resources.tasks.iter().filter(|t|t.target==target).map(|t|t.key.clone()).collect::<Vec<_>>();
                for key in keys{game.resources.supersede(&key).map_err(|m|self.scene_resource_error(m))?;}
            }
        }
        if self.narrative.is_some() {
            next.scene_execution
                .as_mut()
                .unwrap()
                .sprite_styles
                .remove(id);
            if !from.as_ref().is_some_and(|sprite| sprite.visible) {
                crate::scene_resources::replace_target(
                    &mut next,
                    crate::scene_tasks::TaskOwner::Game,
                    crate::scene_resources::TargetKind::Sprite,
                    id,
                )
                .map_err(|message| self.scene_resource_error(message))?;
            } else if let Some(target) = next
                .scene_execution
                .as_ref()
                .unwrap()
                .resources
                .target(
                    &crate::scene_tasks::TaskOwner::Game,
                    crate::scene_resources::TargetKind::Sprite,
                    id,
                )
                .cloned()
            {
                for channel in [
                    crate::scene_resources::EffectChannel::SpriteAnimation,
                    crate::scene_resources::EffectChannel::VisualFade,
                ] {
                    crate::scene_resources::supersede_effect_channel(&mut next, &target, channel)
                        .map_err(|message| self.scene_resource_error(message))?;
                }
            }
            crate::scene_resources::sync_current(&mut next)
                .map_err(|message| self.scene_resource_error(message))?;
            self.plan_retired_transition(&mut next, source, &transition)?;
            self.plan_incoming_transition(&mut next, &slot, &transition)?;
            next.pc += 1;
            return self.commit_ui_state(next);
        }
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
        let Some(from) = from else {
            if self.narrative.is_some() {
                let mut next = self.state.clone();
                next.pc += 1;
                return self.commit_scene_state(next);
            }
            return Ok(());
        };
        let slot = crate::retired_presentations::PresentationSlot::Sprite { id: id.into() };
        let source = if self.narrative.is_some() {
            crate::retired_presentations::snapshot_retired_source(&self.state, &slot)
                .map_err(|message| self.scene_resource_error(message))?
        } else {
            None
        };
        let mut next = self.state.clone();
        if self.narrative.is_some() {
            crate::scene_resources::cancel_retired_story_slot(
                &mut next,
                &slot,
                crate::scene_resources::CancellationReason::TargetRemoved,
            )
            .map_err(|message| self.scene_resource_error(message))?;
        }
        next.last_transition = transition.clone();
        next.motions.tracks.retain(|_,track|!matches!(&track.target,crate::motion::MotionTarget::Sprite{id:target}|crate::motion::MotionTarget::Layer{id:target,..} if target==id));
        if let Some(s) = next.sprites.get_mut(id) {
            s.visible = false;
        }
        if self.narrative.is_some() {
            next.scene_execution
                .as_mut()
                .unwrap()
                .sprite_styles
                .remove(id);
            crate::scene_resources::remove_target(
                &mut next,
                &crate::scene_tasks::TaskOwner::Game,
                crate::scene_resources::TargetKind::Sprite,
                id,
            )
            .map_err(|message| self.scene_resource_error(message))?;
            let layers: Vec<_> = next
                .scene_execution
                .as_ref()
                .unwrap()
                .resources
                .targets
                .iter()
                .filter(|record| {
                    record.owner == crate::scene_tasks::TaskOwner::Game
                        && record.identity.kind == crate::scene_resources::TargetKind::Layer
                        && record.identity.alias.starts_with(&format!("{id}/"))
                })
                .map(|record| record.identity.alias.clone())
                .collect();
            for alias in layers {
                crate::scene_resources::remove_target(
                    &mut next,
                    &crate::scene_tasks::TaskOwner::Game,
                    crate::scene_resources::TargetKind::Layer,
                    &alias,
                )
                .map_err(|message| self.scene_resource_error(message))?;
            }
            crate::scene_resources::sync_current(&mut next)
                .map_err(|message| self.scene_resource_error(message))?;
            self.plan_retired_transition(&mut next, source, &transition)?;
            next.pc += 1;
            return self.commit_ui_state(next);
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
        if self.narrative.is_some() {
            if let Some(target) = next
                .scene_execution
                .as_ref()
                .unwrap()
                .resources
                .target(
                    &crate::scene_tasks::TaskOwner::Game,
                    crate::scene_resources::TargetKind::Sprite,
                    id,
                )
                .cloned()
            {
                crate::scene_resources::supersede_effect_channel(
                    &mut next,
                    &target,
                    crate::scene_resources::EffectChannel::SpriteAnimation,
                )
                .map_err(|message| self.scene_resource_error(message))?;
            }
            next.pc += 1;
            return self.commit_ui_state(next);
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
        if self.consumer_program.validate_emitted(&self.script).is_err()
            || self.history.entries().last().is_some_and(|entry|self.consumer_program.validate_state_binding(&entry.state).is_err()) {
            return false;
        }
        if self.narrative.is_none()&&(self.state.game_audio.is_some()||self.state.game_tasks.is_some()){
            let Some(entry)=self.history.entries().last()else{return false;};let Ok(batch)=self.standalone_audio_restore_batch(entry.state.clone())else{return false;};
            return self.commit_widget_history_replacement(batch,1,true).is_ok();
        }
        if self.narrative.is_none()&&self.state.ui.instance_registry.is_some(){
            let Some(entry)=self.history.entries().last()else{return false;};
            let Some(runtime)=self.runtime_epoch.checked_add(1)else{return false;};let Some(interface)=self.interface_epoch.checked_add(1)else{return false;};let Ok(video)=self.next_video_epoch()else{return false;};let Some(display)=self.display_generation.checked_add(1)else{return false;};
            let Ok(mut batch)=self.build_scene_batch(entry.state.clone(),runtime,interface,video)else{return false;};
            let mut candidate=self.debugger_candidate(crate::eval::debugger_eval::Probe::shared());candidate.functions.debug_probe=None;candidate.state=batch.state.clone();
            match candidate.current_interaction(){Ok(Some(interaction))=>batch.display=Some(interaction),Err(_)=>return false,_=>{}}
            if self.renderer.validate_scene_batch(&batch).is_err(){return false;}
            let _=display;return self.commit_widget_history_replacement(batch,1,true).is_ok();
        }
        if self.narrative.is_some() {
            let Some(entry) = self.history.entries().last() else {
                return false;
            };
            let Ok(batch) = self.scene_restore_batch(entry.state.clone()) else {
                return false;
            };
            return self.commit_widget_history_replacement(batch,1,false).is_ok();
        }
        let Some(display_generation)=self.display_generation.checked_add(1) else{return false;};
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
        let Ok(videos) = self.describe_videos(&entry.state, epoch, self.runtime_epoch) else {
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
        self.restore_legacy_preview_visit(display_generation);
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
        if Self::consumer_choice_options(stmt).is_some()||Self::consumer_dialogue_parts(stmt).is_some()||Self::consumer_imagemap_parts(stmt).is_some(){return true;}
        matches!(
            stmt,
            Statement::Dialogue { .. } | Statement::Choice { .. } | Statement::Imagemap { .. }
        )
    }

    fn interpolate_display(
        &self,
        text: &rvn_parser::InterpolatedText,
    ) -> crate::eval::EvalResult<String> {
        self.functions.interpolate_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state).map_err(|error|EvalError::InvalidFunction(error.to_string()))?.as_ref(), 
            text,
            &self.vars_for_eval(),
            &self.lexical_vars_for_eval(),
            &mut self.state.display_random.clone(), self.locale.as_ref(),
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
                for index in self.visible_choice_indices(options)? {
                    let opt = &options[index];
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
        for index in self.visible_choice_indices(options)? {
            let opt = &options[index];
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

    fn visible_choice_indices(
        &self,
        options: &[rvn_parser::ChoiceOption],
    ) -> Result<Vec<usize>, RuntimeError> {
        let vars = self.vars_for_eval();
        let widget_objects = self.widget_object_context(&self.state)?;
        options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| {
                let visible = match &option.condition {
                    Some(condition) => self
                        .functions
                        .eval_bool_scoped_with_locale_with_widget_objects(widget_objects.as_ref(), 
                            condition,
                            &vars,
                            &self.lexical_vars_for_eval(),
                            &mut self.state.display_random.clone(), self.locale.as_ref(),
                        )
                        .map_err(|error| self.eval_err(error, "Choice (condition)")),
                    None => Ok(true),
                };
                match visible {
                    Ok(true) => Some(Ok(index)),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect()
    }

    /// Authored destinations for the currently visible options. Public choice
    /// submission still takes a visible index; receipts use this identity to
    /// reject a changed list even when all displayed labels are identical.
    pub fn active_choice_indices(&self) -> Result<Vec<usize>, RuntimeError> {
        if self.script.get(self.state.pc).and_then(Self::consumer_choice_options).is_some(){return Ok(self.cached_consumer_choice().map(|c|c.indices()).unwrap_or_default());}
        if let Some((_,_,hotspots))=self.script.get(self.state.pc).and_then(Self::consumer_imagemap_parts){return Ok(if self.cached_consumer_imagemap().is_some(){(0..hotspots.len()).collect()}else{vec![]});}
        match self.script.get(self.state.pc) {
            Some(Statement::Choice { options }) => self.visible_choice_indices(options),
            Some(Statement::Imagemap { hotspots,.. }) => Ok((0..hotspots.len()).collect()),
            _ => Ok(Vec::new()),
        }
    }

    fn record_interaction_snapshot(&mut self, stmt: &Statement) -> Result<(), RuntimeError> {
        if Self::consumer_dialogue_parts(stmt).is_some(){return self.record_consumer_dialogue(stmt);}
        if Self::consumer_choice_options(stmt).is_some(){return self.record_consumer_choice(stmt);}
        if Self::consumer_imagemap_parts(stmt).is_some(){return self.record_consumer_selection(stmt);}
        // Refuse exhaustion before sync, random sampling, history or renderer effects.
        let generation = self.display_generation.checked_add(1)
            .ok_or_else(|| self.eval_err(EvalError::NumericOverflow, "Display generation"))?;
        self.sync_narrative_position();
        let previous = self.narrative.as_ref().map(|_| self.state.clone());
        self.state.consumer_imagemap = None;
        if self.state.display_random_pc != Some(self.state.pc) {
            self.state.display_random =
                crate::random::RandomState::seeded(self.state.random.next_u64());
            self.state.display_random_pc = Some(self.state.pc);
        }
        if matches!(stmt, Statement::Dialogue { .. }) {
            self.state.last_dialogue = Some(crate::types::DialogueSnapshot {
                consumer_dialogue: None,
                pc: self.state.pc,
                vars: self.vars_for_eval(),
                lexical_globals: self
                    .narrative
                    .as_ref()
                    .map(|_| self.lexical_vars_for_eval()),
                random: self.state.display_random,
            });
        }
        self.state.current_interactive_pc = self.state.pc;
        let display = match self.make_display_resolved(stmt) {
            Ok(display) => display,
            Err(error) => {
                if let Some(previous) = previous {
                    self.state = previous;
                }
                return Err(error);
            }
        };
        if let Some(program) = &self.narrative {
            let variables = self.vars_for_eval();
            let lexical_globals = self.lexical_vars_for_eval();
            let choice_indices = match stmt {
                Statement::Choice { options } => self.visible_choice_indices(options)?,
                _ => Vec::new(),
            };
            let checkpoint = crate::scene::DisplayCheckpoint {
                consumer_dialogue: None,
                consumer_choice:None,consumer_imagemap:None,
                position: program.position(self.state.pc),
                variables,
                lexical_globals,
                random: self.state.display_random,
                typewriter: self.state.typewriter.clone(),
                display: display.clone().expect("interactive display"),
                choice_indices,
                reveal: None,
            };
            self.state.scene_execution.as_mut().unwrap().display = Some(checkpoint);
            self.display_owner = self.state.scene_execution.as_ref().and_then(|execution| execution.owner());
        }
        self.display_generation = generation;
        self.history.push_audio_entry(self.state.clone(), display);
        self.display_history_entry = if self.narrative.is_some()||self.state.game_audio.is_some() { self.history.latest_entry_id() } else { None };
        if self.state.game_audio.is_some(){self.history.arm_audio_publication(&self.state);}
        self.legacy_preview_visit = if self.narrative.is_none(){Some(LegacyPreviewVisit::capture(&self.state,self.history.latest_entry_id()))}else{None};
        Ok(())
    }

    fn exec_interactive(&mut self, stmt: Statement) -> Result<(), RuntimeError> {
        self.exec_interactive_inner(stmt)?;
        self.sync_narrative_position();
        self.arm_interactive_audio_publication()?;
        Ok(())
    }

    fn exec_interactive_inner(&mut self, stmt: Statement) -> Result<(), RuntimeError> {
        if Self::consumer_dialogue_parts(&stmt).is_some(){if let Some(Interaction::Dialogue{character,text})=self.current_interaction()?{self.renderer.show_dialogue(character.as_deref(),&text);return self.advance_consumer_dialogue();}return Ok(());}
        if Self::consumer_choice_options(&stmt).is_some(){if let Some(Interaction::Choice{options})=self.current_interaction()?{let selected=self.renderer.show_choice(&options);return self.submit_choice(selected);}return Ok(());}
        if Self::consumer_imagemap_parts(&stmt).is_some(){if let Some(Interaction::Imagemap{background,hover_image,hotspots})=self.current_interaction()?{let selected=self.renderer.show_imagemap(&background,hover_image.as_deref(),&hotspots);return self.submit_hotspot(selected);}return Ok(());}
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
                let active = self.visible_choice_indices(&options)?;
                if active.is_empty() {
                    self.state.pc += 1;
                    return Ok(());
                }
                let labels = self.resolve_choice_labels(&options)?;
                let selected = self.renderer.show_choice(&labels);
                let idx = selected.min(active.len().saturating_sub(1));
                let body = options[active[idx]].body.clone();
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
        if self.startup.is_some()&&self.initializing_consumers&&matches!(stmt,Statement::ScheduledConsumer{..}) {
            let probe=self.functions.debug_probe.clone().ok_or_else(||self.scene_resource_error("Initializer producer lost its actual trace owner".into()))?;
            let route=self.initializer_consumer_route(&stmt)?;
            crate::eval::debugger_eval::Probe::initializer_route(&probe,route,||self.exec_silent_inner(stmt)).map_err(|e|self.eval_err(e,"Initializer producer route"))??;
        }else{self.exec_silent_inner(stmt)?;}
        self.sync_narrative_position();
        Ok(())
    }

    fn exec_scheduled_consumer(&mut self, statement: Statement) -> Result<(),RuntimeError> {
        use crate::consumer_schedule::{BoundConsumerFrame,ConsumerOwner};
        self.consumer_program.validate_plan_binding(&statement,self.state.pc,self.initializing_consumers)
            .map_err(|error|self.eval_err(error,"Scheduled consumer plan binding"))?;
        if self.initializing_consumers&&self.startup.is_some()&&Self::is_owned_startup_control(&statement){return self.exec_startup_control(statement);}
        if !self.initializing_consumers&&matches!(statement,Statement::ScheduledConsumer{version:3,..}){return self.exec_narrative_native_loop(statement);}
        if Self::consumer_imagemap_parts(&statement).is_some(){
            if self.initializing_consumers&&self.startup.is_some(){return self.exec_startup_imagemap_header(&statement);}
            return self.record_consumer_selection(&statement);
        }
        let Statement::ScheduledConsumer {scope,body,..} = &statement else {unreachable!()};
        if let [leaf]=body.as_slice(){
            if rvn_parser::widget_viewport_effect_1003::parts(leaf).is_some(){return self.exec_widget_viewport_consumer(statement);}
            if rvn_parser::widget_create_effect::parts(leaf).map_err(|error|self.scene_resource_error(error))?.is_some(){
                return self.exec_widget_factory_consumer(statement);
            }
        }
        if !matches!(body.as_slice(),[Statement::SetVar{..}|Statement::LocalVar{..}]){return self.exec_scheduled_action(statement);}
        let (name,local) = match &body[0] {
            Statement::SetVar {name,..} => (name.clone(),false),
            Statement::LocalVar {name,..} => (name.clone(),true),
            _ => return Err(self.eval_err(EvalError::InvalidFunction("Scheduled return requires a function activation".into()),"Scheduled consumer")),
        };
        let persistent_binding = !local && Self::is_persistent(&name)
            && name.strip_prefix("persistent.").is_some_and(rvn_parser::is_variable_binding_name);
        let compiled_temporary=!local&&(self.consumer_temporary_writes.get(&self.state.pc)==Some(&name)||self.narrative.as_ref().is_some_and(|p|p.temporary_writes.get(&self.state.pc)==Some(&name)));
        if !rvn_parser::is_variable_binding_name(&name) && !persistent_binding && !compiled_temporary {
            return Err(self.eval_err(EvalError::InvalidFunction("Invalid scheduled consumer variable binding".into()),"Scheduled consumer"));
        }
        let owner = self.state.scene_execution.as_ref().and_then(|execution|execution.owner())
            .map(|activation|ConsumerOwner::Scene {activation}).unwrap_or(ConsumerOwner::Game);
        if local && owner == ConsumerOwner::Game {
            return Err(self.eval_err(EvalError::InvalidFunction("local narratif sans scène active".into()),"Scheduled LocalVar"));
        }
        let environment = self.vars_for_eval();
        let lexical_globals = self.lexical_vars_for_eval();
        let scope = self.consumer_program.bound_scope(scope,self.consumer_init_pc.unwrap_or(self.state.pc)).map_err(|error|self.eval_err(error,"Scheduled namespace"))?;
        let program_identity = self.consumer_program.identity().map_err(|error|self.eval_err(error,"Scheduled program identity"))?;
        let mut next = self.state.clone();
        let slot = match next.scene_execution.as_mut().and_then(|execution|execution.active.as_mut()) {
            Some(active) => &mut active.consumer_frame,
            None => &mut next.consumer_frame,
        };
        let frame = slot.get_or_insert_with(||BoundConsumerFrame::new(program_identity,owner));
        frame.validate_binding(program_identity,owner)
            .map_err(|error|self.eval_err(error,"Scheduled consumer frame binding"))?;
        let value = self.functions.eval_engine_consumer_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &statement,&environment,&lexical_globals,&mut next.random,&mut frame.frame,&scope,self.locale.as_ref())
            .map_err(|error|self.eval_err(error,"Scheduled consumer candidate"))?;
        let persistent = Self::is_persistent(&name);
        let persistent_update = if persistent { Some((name.clone(),value.clone())) } else { None };
        if local {
            next.scene_execution.as_mut().unwrap().active.as_mut().unwrap().locals.insert(name,value);
        } else if !persistent {
            if let Some(execution) = &mut next.scene_execution {
                if self.narrative.as_ref().is_some_and(|program|program.temporary_writes.get(&self.state.pc)==Some(&name)) {
                    execution.temporaries_mut().insert(name,value);
                } else if execution.active.as_ref().is_some_and(|active|active.locals.contains_key(&name)) {
                    execution.active.as_mut().unwrap().locals.insert(name,value);
                } else { next.vars.insert(name,value); }
            } else { next.vars.insert(name,value); }
        }
        next.pc += 1;
        // The lexical persistent candidate is visible to renderer preflight;
        // an error restores it before returning and publishes no state/frame/RNG.
        let prior_persistent = persistent_update.map(|(name,value)| {
            let prior = self.persistent_vars.insert(name.clone(),value); (name,prior)
        });
        let result = if self.narrative.is_some() {
            self.commit_scene_state(next)
        } else if !next.ui.screens.is_empty() {
            self.commit_ui_state(next)
        } else { self.state=next; Ok(()) };
        if result.is_err() {
            if let Some((name,prior)) = prior_persistent {
                match prior { Some(value) => {self.persistent_vars.insert(name,value);}, None => {self.persistent_vars.remove(&name);} }
            }
        }
        result
    }


    /// A real consumer finishes its direct gather before any selected child
    /// instruction. All effects remain in next until the normal atomic commit.
    pub(in crate::engine) fn gather_scheduled_action_inputs(&self,statement:&Statement)->Result<(GameState,Value),RuntimeError>{
        self.gather_scheduled_action_inputs_in(statement,self.vars_for_eval())
    }
    pub(in crate::engine) fn gather_scheduled_action_inputs_in(&self,statement:&Statement,environment:HashMap<String,Value>)->Result<(GameState,Value),RuntimeError>{
        use crate::consumer_schedule::{BoundConsumerFrame,ConsumerOwner};
        let Statement::ScheduledConsumer{scope,..}=statement else{unreachable!()};
        let owner=self.state.scene_execution.as_ref().and_then(|e|e.owner()).map(|activation|ConsumerOwner::Scene{activation}).unwrap_or(ConsumerOwner::Game);
        let scope=self.consumer_program.bound_scope(scope,self.consumer_init_pc.unwrap_or(self.state.pc)).map_err(|e|self.eval_err(e,"Scheduled namespace"))?;
        let identity=self.consumer_program.identity().map_err(|e|self.eval_err(e,"Scheduled identity"))?;
        let globals=self.lexical_vars_for_eval();let mut next=self.state.clone();
        let slot=match next.scene_execution.as_mut().and_then(|e|e.active.as_mut()){Some(a)=>&mut a.consumer_frame,None=>&mut next.consumer_frame};
        let frame=slot.get_or_insert_with(||BoundConsumerFrame::new(identity,owner));frame.validate_binding(identity,owner).map_err(|e|self.eval_err(e,"Scheduled owner"))?;
        let values=self.functions.eval_engine_consumer_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &statement,&environment,&globals,&mut next.random,&mut frame.frame,&scope,self.locale.as_ref()).map_err(|e|self.eval_err(e,"Scheduled direct inputs"))?;
        Ok((next,values))
    }
    fn exec_scheduled_action(&mut self,statement:Statement)->Result<(),RuntimeError>{
        let Statement::ScheduledConsumer{body,..}=&statement else{unreachable!()};
        let(mut next,values)=self.gather_scheduled_action_inputs(&statement)?;
        let globals=self.lexical_vars_for_eval();
        match body.as_slice(){
            [Statement::While{body,..}]=>{
                let mut work=0;let entered=crate::eval::truthy_bounded(&values,Some(self.functions.text_resources()),&mut work).map_err(|e|self.eval_err(e,"Scheduled While"))?;
                if !entered{next.pc+=1;return self.commit_scheduled_state(next);}
                let[Statement::Call{target}]=body.as_slice()else{return Err(self.eval_err(EvalError::InvalidFunction("Scheduled While requires its actual prepared iteration branch".into()),"Scheduled While"));};
                let destination=self.resolve(target)?;if self.narrative.is_some(){return self.transfer_narrative_from(next,destination,Some(self.state.pc),true);}
                if next.call_stack.len()>=128{return Err(self.eval_err(EvalError::ExecutionLimit{limit:"128 appels narratifs imbriqués"},"Scheduled While"));}next.call_stack.push(self.state.pc);next.pc=destination;self.commit_scheduled_state(next)
            },
            []|[Statement::Sequence{..}] if body.is_empty()||matches!(body.first(),Some(Statement::Sequence{branches})if branches.is_empty())=>{next.pc+=1;self.commit_scheduled_state(next)},
            [Statement::Sequence{branches}] if self.initializing_consumers=>{self.commit_scheduled_state(next)?;for branch in branches{self.exec_block(branch)?;}Ok(())},
            [Statement::If{then_branch,else_branch,..}]=>{
                let mut work=0;let selected=crate::eval::truthy_bounded(&values,Some(self.functions.text_resources()),&mut work).map_err(|e|self.eval_err(e,"Scheduled If"))?;
                let branch=if selected{then_branch}else{else_branch};
                if self.initializing_consumers&&self.startup.is_some(){
                    let route=self.startup.as_ref().unwrap().actual_if_route(&self.script,then_branch,else_branch).map_err(|e|self.scene_resource_error(e))?;
                    self.state=next;
                    self.startup.as_mut().unwrap().enter_if(&self.script,route,selected).map_err(|e|self.scene_resource_error(e))?;
                    return Ok(());
                }
                if self.initializing_consumers{self.commit_scheduled_state(next)?;return self.exec_block(branch);}
                if branch.is_empty(){next.pc+=1;return self.commit_scheduled_state(next);}
                let[Statement::Call{target}]=branch.as_slice()else{return Err(self.eval_err(EvalError::InvalidFunction("Scheduled If requires its actual prepared branch".into()),"Scheduled If"));};
                let destination=self.resolve(target)?;if self.narrative.is_some(){return self.transfer_narrative_from(next,destination,Some(self.state.pc+1),true);}
                if next.call_stack.len()>=128{return Err(self.eval_err(EvalError::ExecutionLimit{limit:"128 appels narratifs imbriqués"},"Scheduled If"));}next.call_stack.push(self.state.pc+1);next.pc=destination;self.commit_scheduled_state(next)
            },
            [Statement::Jump{target}]=>{let destination=self.resolve(target)?;if self.narrative.is_some(){self.transfer_narrative_from(next,destination,None,false)}else{next.pc=destination;self.commit_scheduled_state(next)}},
            [Statement::Call{target}]=>{let destination=self.resolve(target)?;if let Some(program)=&self.narrative{let synthetic=program.generated_targets.contains(target);return self.transfer_narrative_from(next,destination,Some(self.state.pc+1),synthetic);}if next.call_stack.len()>=128{return Err(self.eval_err(EvalError::ExecutionLimit{limit:"128 appels narratifs imbriqués"},"Scheduled Call"));}next.call_stack.push(self.state.pc+1);next.pc=destination;self.commit_scheduled_state(next)},
            [Statement::Return]=>{if self.narrative.is_some(){return self.narrative_return_from(next,self.branch_returns.contains(&self.state.pc));}
                if !self.branch_returns.contains(&self.state.pc){while next.call_stack.last().is_some_and(|pc|{matches!(Self::consumer_leaf(self.script.get(*pc)),Some(Statement::While{..}))||pc.checked_sub(1).is_some_and(|pc|matches!(Self::consumer_leaf(self.script.get(pc)),Some(Statement::If{..}|Statement::Choice{..}|Statement::Imagemap{..})))} ){next.call_stack.pop();}}
                next.pc=next.call_stack.pop().ok_or_else(||RuntimeError::no_stmt(crate::error::RuntimeErrorKind::ReturnWithoutCall,self.state.pc))?;self.commit_scheduled_state(next)
            },
            [Statement::MotionWait{..}]=>{let Value::Str(key)=values else{return Err(self.eval_err(EvalError::InvalidFunction("Animation target must be a string".into()),"Scheduled MotionWait"));};crate::motion::MotionTarget::parse(&key).map_err(|e|self.scene_resource_error(e))?;
                if let Some(track)=next.motions.tracks.get(&key).filter(|t|t.running){if track.definition.validate().map_err(|e|self.scene_resource_error(e))?.seconds.is_none(){return Err(self.scene_resource_error("Cannot wait for an endless animation".into()));}next.motions.waiting=Some(key);if self.narrative.is_none(){next.current_interactive_pc=next.pc;}}
                if self.narrative.is_some()||next.motions.waiting.is_none(){next.pc+=1;}self.commit_scheduled_state(next)
            },
            [Statement::VideoWait{..}]=>{let Value::Str(name)=values else{return Err(self.eval_err(EvalError::InvalidFunction("Video wait requires a player name".into()),"Scheduled VideoWait"));};next.videos.wait(&name).map_err(|e|self.scene_resource_error(e))?;next.pc+=1;self.commit_ui_state(next)},
            [Statement::TypewriterSpeedValue{..}]=>{
                let Value::List(mut ports)=values else{return Err(self.scene_resource_error("Dynamic typewriter speed lost its real direct-input tuple".into()));};
                if ports.len()!=1{return Err(self.scene_resource_error("Dynamic typewriter speed requires exactly one evaluated input".into()));}
                let chars_per_sec=self.typewriter_speed_value(ports.remove(0))?;
                self.exec_scheduled_classic(next,Statement::TypewriterSpeed{chars_per_sec},classic_consumer::ClassicOrigin::Native)
            },
            [Statement::TypewriterSetValue{..}]=>{
                let Value::List(mut ports)=values else{return Err(self.scene_resource_error("Dynamic typewriter Boolean lost its real direct-input tuple".into()));};
                if ports.len()!=1{return Err(self.scene_resource_error("Dynamic typewriter Boolean requires exactly one evaluated input".into()));}
                let enabled=self.typewriter_enabled_value(ports.remove(0))?;
                self.exec_scheduled_classic(next,Statement::TypewriterSet{enabled},classic_consumer::ClassicOrigin::Native)
            },
            [Statement::MusicVolumeValue{..}]=>{
                let Value::List(mut ports)=values else{return Err(self.scene_resource_error("Dynamic music volume lost its real direct-input tuple".into()));};
                if ports.len()!=1{return Err(self.scene_resource_error("Dynamic music volume requires exactly one evaluated input".into()));}
                let level=self.music_volume_value(ports.remove(0))?;
                next.ensure_game_audio().map_err(|m|self.scene_resource_error(m))?;
                self.exec_narrative_music_volume_from(next,level)
            },
            [command] if crate::game_audio::is_audio_command(command)=>{
                if !matches!(values,Value::List(ref ports) if ports.is_empty()){return Err(self.scene_resource_error("Literal audio command lost its exact empty direct-input tuple".into()));}
                next.ensure_game_audio().map_err(|m|self.scene_resource_error(m))?;
                self.exec_scheduled_audio(next,command.clone())
            },
            [command] if classic_consumer::is_classic(command)=>{
                if !matches!(values,Value::List(ref ports) if ports.is_empty()) {
                    return Err(self.scene_resource_error("Literal classic command lost its exact empty direct-input tuple".into()));
                }
                self.exec_scheduled_classic(next,command.clone(),if matches!(&statement,Statement::ScheduledConsumer{version:2,..}){classic_consumer::ClassicOrigin::Native}else{classic_consumer::ClassicOrigin::Historical})
            },
            [Statement::MenuExecute{..}]=>Err(self.eval_err(EvalError::InvalidFunction("Game-menu requests require a trusted interface event source".into()),"Scheduled Menu")),
            [command]=>{let Value::List(values)=values else{return Err(self.eval_err(EvalError::InvalidFunction("Scheduled command lost its actual input tuple".into()),"Scheduled command"));};let(command,bindings)=crate::eval::debugger_eval::replace_command_values(command.clone(),values).map_err(|e|self.eval_err(e,"Scheduled command fields"))?;
                // Values are already evaluated. This scratch validation cannot
                // create debugger roots for implementation-only variable names.
                let mut library=self.functions.clone();library.debug_probe=None;
                let command=crate::eval::UiCommand::evaluate_scoped_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &library,&command,&bindings,&globals,&mut next.random,self.locale.as_ref()).map_err(|e|self.eval_err(e,"Scheduled command"))?;
                self.apply_ui_commands(&mut next,vec![command])?;next.pc+=1;self.commit_ui_state(next)
            },_=>Err(self.eval_err(EvalError::InvalidFunction("Scheduled control requires actual lowering/owner".into()),"Scheduled consumer")),
        }
    }
    fn commit_scheduled_state(&mut self,next:GameState)->Result<(),RuntimeError>{if next.game_audio.is_some()||next.game_tasks.is_some(){return self.commit_audio_state(next);}if self.narrative.is_some(){self.commit_scene_state(next)}else if !next.ui.screens.is_empty(){self.commit_ui_state(next)}else{self.state=next;Ok(())}}
    fn consumer_leaf(statement:Option<&Statement>)->Option<&Statement>{match statement{Some(Statement::ScheduledConsumer{version:2,body,..})if body.len()==1=>body.first(),other=>other}}

    fn exec_silent_inner(&mut self, stmt: Statement) -> Result<(), RuntimeError> {
        self.with_host_random_transaction(|engine| engine.exec_silent_inner_host_planned(stmt))
    }

    fn exec_silent_inner_host_planned(&mut self, stmt: Statement) -> Result<(), RuntimeError> {
        match stmt {
            Statement::UnitCall{name,arguments} => return self.exec_unit_call(&name,&arguments),
            Statement::LibraryUnitCall{call} => return self.exec_library_unit_call1023(&call),
            Statement::UnitReturn => return Err(self.eval_err(EvalError::InvalidFunction("Unit return has no active synchronous function invocation".into()),"Unit return")),
            Statement::SceneLabel { .. } => {
                return self.transfer_narrative(self.state.pc, None, false);
            }
            Statement::Use { .. }
            | Statement::Init { .. }
            | Statement::Label { .. }
            | Statement::Function { .. }
            | Statement::FunctionLibrary { .. }
            | Statement::UnitFunction { .. }
            | Statement::Screen { .. }
            | Statement::Handler { .. } => {
                self.state.pc += 1;
            }
            Statement::LoopBreak => {
                if self.narrative.as_ref().is_some_and(|program|!program.native_loops.is_empty()){
                    return self.exec_narrative_native_break();
                }
                return Err(self.eval_err(crate::eval::EvalError::InvalidFunction("break non abaissé hors ForLoopWithBreak".into()),"flow.break"));
            }
            Statement::FunctionReturn { .. } => {
                return Err(self.eval_err(
                    EvalError::InvalidFunction("return <valeur> hors d’une fonction".into()),
                    "FunctionReturn",
                ))
            }
            Statement::LocalVar { name, value } => {
                // Reject before evaluating the initializer (which may consume
                // random values) when a helper is called before any scene.
                if !self
                    .state
                    .scene_execution
                    .as_ref()
                    .is_some_and(|execution| execution.active.is_some())
                {
                    return Err(self.eval_err(
                        EvalError::InvalidFunction("local narratif sans scène active".into()),
                        "LocalVar",
                    ));
                }
                let mut next = self.state.clone();
                let value = self
                    .functions
                    .eval_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                        &value,
                        &self.vars_for_eval(),
                        &self.lexical_vars_for_eval(),
                        &mut next.random, self.locale.as_ref(),
                    )
                    .map_err(|error| self.eval_err(error, "LocalVar"))?;
                next.scene_execution
                    .as_mut()
                    .unwrap()
                    .active
                    .as_mut()
                    .unwrap()
                    .locals
                    .insert(name, value);
                next.pc += 1;
                return self.commit_scene_state(next);
            }
            statement @ Statement::ScheduledConsumer {..} => return self.exec_scheduled_consumer(statement),
            Statement::MenuExecute { .. } => {
                return Err(self.eval_err(
                    EvalError::InvalidFunction(
                        "Game-menu requests require a trusted interface event".into(),
                    ),
                    "Menu execute",
                ))
            }
            Statement::WidgetAddToViewport { .. } | Statement::WidgetRemoveFromParent { .. } => return Err(self.scene_resource_error("Widget viewport effect requires its actual reached Native Scene consumer".into())),
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
                let environment = if self.narrative.is_some() {
                    self.vars_for_eval()
                } else {
                    next.vars.clone()
                };
                let lexical_globals = if self.narrative.is_some() {
                    self.lexical_vars_for_eval()
                } else {
                    next.vars.clone()
                };
                let command = crate::eval::UiCommand::evaluate_scoped_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                    &self.functions,
                    &statement,
                    &environment,
                    &lexical_globals,
                    &mut next.random, self.locale.as_ref(),
                )
                .map_err(|error| self.eval_err(error, "Interface"))?;
                self.apply_ui_commands(&mut next, vec![command])?;
                if self.narrative.is_some() {
                    next.pc += 1;
                    return self.commit_ui_state(next);
                }
                self.commit_ui_state(next)?;
                self.state.pc += 1;
            }
            Statement::VideoWait { name } => {
                let mut next = self.state.clone();
                let Value::Str(name) = self
                    .functions
                    .eval_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                        &name,
                        &self.vars_for_eval(),
                        &self.lexical_vars_for_eval(),
                        &mut next.random, self.locale.as_ref(),
                    )
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
                    .eval_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                        &target,
                        &self.vars_for_eval(),
                        &self.lexical_vars_for_eval(),
                        &mut random, self.locale.as_ref(),
                    )
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
                if self.narrative.is_some() {
                    let mut next = self.state.clone();
                    if let Some(track) = next.motions.tracks.get(&key).filter(|track| track.running)
                    {
                        if track
                            .definition
                            .validate()
                            .map_err(|message| self.scene_resource_error(message))?
                            .seconds
                            .is_none()
                        {
                            return Err(self.scene_resource_error(
                                "Cannot wait for an endless animation; use a finite repeat count"
                                    .into(),
                            ));
                        }
                        next.motions.waiting = Some(key);
                    }
                    next.random = random;
                    next.pc += 1;
                    return self.commit_scene_state(next);
                }
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
                if self.narrative.is_some() {
                    let mut next = self.state.clone();
                    let entered = self
                        .functions
                        .eval_bool_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                            &condition,
                            &self.vars_for_eval(),
                            &self.lexical_vars_for_eval(),
                            &mut next.random, self.locale.as_ref(),
                        )
                        .map_err(|error| self.eval_err(error, "While"))?;
                    if entered {
                        let [Statement::Call { target }] = body.as_slice() else {
                            return Err(self.eval_err(
                                EvalError::InvalidFunction("Prepared while body is invalid".into()),
                                "While",
                            ));
                        };
                        return self.transfer_narrative_from(
                            next,
                            self.resolve(target)?,
                            Some(self.state.pc),
                            true,
                        );
                    }
                    next.pc += 1;
                    return self.commit_scene_state(next);
                }
                if self
                    .functions
                    .eval_bool_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                        &condition,
                        &self.vars_for_eval(),
                        &mut self.state.random, self.locale.as_ref(),
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
            Statement::StandardMacroDispatch{..} => {
                return Err(self.eval_err(EvalError::InvalidFunction("StandardMacro requires an admitted actual Widget Handler owner".into()),"StandardMacro"));
            }
            statement @ (Statement::Sequence{..}|Statement::ForLoopWithBreak{..}) => {
                let original=self.vars_for_eval();let mut next=self.state.clone();
                let computed=self.functions.execute_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &[statement],&original,&mut next.random, self.locale.as_ref()).map_err(|e|self.eval_err(e,"Structured flow"))?;
                for(name,value)in computed {if original.get(&name)!=Some(&value){next.vars.insert(name,value);}}
                next.pc+=1;
                if self.narrative.is_some(){return self.commit_scene_state(next);}self.state=next;
            }
            statement @ (Statement::While { .. } | Statement::ForEach { .. }) => {
                let original = self.vars_for_eval();
                let computed = self
                    .functions
                    .execute_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &[statement], &original, &mut self.state.random, self.locale.as_ref())
                    .map_err(|error| self.eval_err(error, "Loop"))?;
                for (name, value) in computed {
                    if original.get(&name) != Some(&value) {
                        self.state.vars.insert(name, value);
                    }
                }
                self.state.pc += 1;
            }
            Statement::Jump { target } => {
                if self.narrative.is_some() {
                    return self.transfer_narrative(self.resolve(&target)?, None, false);
                }
                self.state.pc = self.resolve(&target)?;
            }
            Statement::Call { target } => {
                if let Some(program) = &self.narrative {
                    let synthetic = program.generated_targets.contains(&target);
                    return self.transfer_narrative(
                        self.resolve(&target)?,
                        Some(self.state.pc + 1),
                        synthetic,
                    );
                }
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
                if self.narrative.is_some() {
                    return self.narrative_return(self.branch_returns.contains(&self.state.pc));
                }
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
            command @ (Statement::Config{..}|Statement::Scene{..}|Statement::CinematicShow{..}|Statement::CinematicHide{..}|Statement::UnlockEnding{..}|Statement::ShowSprite{..}|Statement::HideSprite{..}|Statement::MoveSprite{..}|Statement::SpriteAnimate{..}|Statement::SpriteStopAnimation{..}|Statement::SpriteEffect{..})=>return self.exec_classic_body(command),
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
            command @ Statement::CharacterCreate { .. } => return self.exec_classic_body(command),
            Statement::SetVar { name, value } => {
                let mut random = self.state.random;
                let val = self
                    .functions
                    .eval_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                        &value,
                        &self.vars_for_eval(),
                        &self.lexical_vars_for_eval(),
                        &mut random, self.locale.as_ref(),
                    )
                    .map_err(|e| self.eval_err(e, &format!("SetVar {{ name: {:?} }}", name)))?;
                if let Some(program) = &self.narrative {
                    if !Self::is_persistent(&name) {
                        let mut next = self.state.clone();
                        let execution = next.scene_execution.as_mut().unwrap();
                        if program.temporary_writes.get(&self.state.pc) == Some(&name) {
                            execution.temporaries_mut().insert(name, val);
                        } else if execution
                            .active
                            .as_ref()
                            .is_some_and(|active| active.locals.contains_key(&name))
                        {
                            execution.active.as_mut().unwrap().locals.insert(name, val);
                        } else {
                            next.vars.insert(name, val);
                        }
                        next.random = random;
                        next.pc += 1;
                        return self.commit_scene_state(next);
                    }
                }
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
                if self.initializing_consumers{
                    if self.startup.is_some(){return self.execute_startup_if(condition,&then_branch,&else_branch);}
                    // Init retains authored nested bodies; narrative Ifs below
                    // have already been lowered to real prepared Call sites.
                    let globals=self.vars_for_eval();let lexical=self.lexical_vars_for_eval();
                    let selected=self.functions.eval_bool_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &condition,&globals,&lexical,&mut self.state.random,self.locale.as_ref()).map_err(|error|self.eval_err(error,"Initializer If condition"))?;
                    self.exec_block(if selected{&then_branch}else{&else_branch})?;
                    self.state.pc+=1;return Ok(());
                }
                if self.narrative.is_some() {
                    let mut next = self.state.clone();
                    let selected = self
                        .functions
                        .eval_bool_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                            &condition,
                            &self.vars_for_eval(),
                            &self.lexical_vars_for_eval(),
                            &mut next.random, self.locale.as_ref(),
                        )
                        .map_err(|error| self.eval_err(error, "If"))?;
                    let branch = if selected { then_branch } else { else_branch };
                    if let Some(Statement::Call { target }) = branch.first() {
                        return self.transfer_narrative_from(
                            next,
                            self.resolve(target)?,
                            Some(self.state.pc + 1),
                            true,
                        );
                    }
                    if !branch.is_empty() {
                        return Err(self.eval_err(
                            EvalError::InvalidFunction("Prepared if body is invalid".into()),
                            "If",
                        ));
                    }
                    next.pc += 1;
                    return self.commit_scene_state(next);
                }
                let branch = if self
                    .functions
                    .eval_bool_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                        &condition,
                        &self.vars_for_eval(),
                        &mut self.state.random, self.locale.as_ref(),
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
                if self.narrative.is_some()||self.state.game_audio.is_some() {
                    return self.exec_narrative_music_play(resolved, transition);
                }
                let previous = self.state.music.current_file.clone();
                self.state.music.current_file = Some(resolved.clone());
                self.renderer
                    .music_play(&resolved, &transition, previous.as_deref());
                self.state.pc += 1;
            }
            Statement::MusicStop { transition } => {
                if self.narrative.is_some()||self.state.game_audio.is_some() {
                    return self.exec_narrative_music_stop(transition);
                }
                self.state.music.current_file = None;
                self.renderer.music_stop(&transition);
                self.state.pc += 1;
            }
            Statement::TypewriterSpeedValue { chars_per_sec } => {
                let mut next=self.state.clone();
                let value=self.functions.eval_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &chars_per_sec,&self.vars_for_eval(),&self.lexical_vars_for_eval(),&mut next.random,self.locale.as_ref())
                    .map_err(|e|self.eval_err(e,"Typewriter speed"))?;
                let chars_per_sec=self.typewriter_speed_value(value)?;
                return self.commit_typewriter_speed(next,chars_per_sec);
            }
            Statement::TypewriterSetValue { enabled } => {
                let mut next=self.state.clone();
                let value=self.functions.eval_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &enabled,&self.vars_for_eval(),&self.lexical_vars_for_eval(),&mut next.random,self.locale.as_ref())
                    .map_err(|e|self.eval_err(e,"Typewriter enabled"))?;
                let enabled=self.typewriter_enabled_value(value)?;
                return self.commit_typewriter_setting(next,enabled);
            }
            Statement::MusicVolumeValue { level } => {
                let mut next=self.state.clone();
                let value=self.functions.eval_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &level,&self.vars_for_eval(),&self.lexical_vars_for_eval(),&mut next.random,self.locale.as_ref())
                    .map_err(|e|self.eval_err(e,"Music volume"))?;
                let level=self.music_volume_value(value)?;
                next.ensure_game_audio().map_err(|m|self.scene_resource_error(m))?;
                return self.exec_narrative_music_volume_from(next,level);
            }
            Statement::MusicVolume { level } => {
                if self.narrative.is_some()||self.state.game_audio.is_some() {
                    return self.exec_narrative_music_volume_from(self.state.clone(),level);
                }
                self.state.music.volume = level;
                self.renderer.music_set_volume(level);
                self.state.pc += 1;
            }
            Statement::SfxPlay { file, transition } => {
                if self.narrative.is_some()||self.state.game_audio.is_some() { let resolved = if file.starts_with("sfx/") { file.clone() } else { format!("sfx/{file}") }; return self.exec_owned_audio_play(crate::audio_playback::AudioFamily::Sfx, resolved, f64::from(transition.duration_ms()) / 1000.0); }
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
                if self.narrative.is_some()||self.state.game_audio.is_some() { let resolved = if file.starts_with("sfx/") { file.clone() } else { format!("sfx/{file}") }; return self.exec_owned_audio_stop(crate::audio_playback::AudioFamily::Sfx, Some(resolved), f64::from(transition.duration_ms()) / 1000.0); }
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
                if self.narrative.is_some()||self.state.game_audio.is_some() { return self.exec_owned_audio_play(crate::audio_playback::AudioFamily::Voice, file.clone(), 0.0); }
                self.renderer.voice_play(&file);
                self.state.pc += 1;
            }
            Statement::VoiceStop => {
                if self.narrative.is_some()||self.state.game_audio.is_some() { return self.exec_owned_audio_stop(crate::audio_playback::AudioFamily::Voice, None, 0.0); }
                self.renderer.voice_stop();
                self.state.pc += 1;
            }
            command @ (Statement::Timer{..}|Statement::TimerCancel|Statement::TypewriterSet{..}|Statement::TypewriterSpeed{..})=>return self.exec_classic_body(command),
            Statement::Dialogue { .. } | Statement::Choice { .. } | Statement::Imagemap { .. } => {
                unreachable!("exec_silent appelé sur un statement interactif")
            }
        }
        Ok(())
    }

    fn exec_block(&mut self, stmts: &[Statement]) -> Result<(), RuntimeError> {
        let saved_pc = self.state.pc;
        for stmt in stmts {
            self.state.pc = if self.startup.is_some()&&self.initializing_consumers {
                self.consumer_init_pc.ok_or_else(||self.scene_resource_error("Startup block lost its genuine Init root".into()))?
            }else if self.narrative.is_some() {
                // Init executes before gameplay. Keep its candidate at a real
                // prepared boundary rather than manufacturing len + 1.
                saved_pc
            } else {
                self.script.len()
            };
            self.exec_silent(stmt.clone())?;
        }
        self.state.pc = saved_pc;
        self.sync_narrative_position();
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
            if self.state.motions.waiting.is_some() || self.state.videos.waiting.is_some() {
                return Ok(None);
            }
            if self.is_finished() {
                return Ok(None);
            }

            let stmt = self.script[self.state.pc].clone();
            if matches!(&stmt, Statement::Choice { options } if !options.is_empty())
                && self.state.display_random_pc != Some(self.state.pc)
            {
                self.state.display_random =
                    crate::random::RandomState::seeded(self.state.random.next_u64());
                self.state.display_random_pc = Some(self.state.pc);
            }
            match &stmt {
                // Ces cas devraient être signalés par `rvn check`, mais le moteur
                // reste robuste et ne bloque pas l'UI si le script les contient.
                Statement::Choice { options }
                    if self.visible_choice_indices(options)?.is_empty() =>
                {
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
                    let prior_pc=self.state.pc;
                    self.record_interaction_snapshot(&stmt)?;
                    if self.state.pc!=prior_pc{continue;}
                    return self.current_interaction();
                }
                _ => return Ok(None),
            }
        }
    }

    /// Publish a reached scene interaction for an event-driven host even when
    /// no intervening statement produced a model batch. This snapshots the
    /// authorized display; it neither executes a statement nor consumes input.
    pub fn publish_current_scene_interaction(&mut self) -> Result<(), RuntimeError> {
        let Some(execution) = self.state.scene_execution.as_ref() else {
            return Ok(());
        };
        if execution.display.as_ref().is_none_or(|display| display.position != execution.live_position)
            || self.display_owner != execution.owner()
        {
            return Err(self.eval_err(
                EvalError::InvalidFunction("Scene interaction publication requires an authorized display at the current position".into()),
                "Scene interaction publication",
            ));
        }
        let program = self.narrative.as_ref().ok_or_else(|| self.eval_err(
            EvalError::InvalidFunction("Scene interaction has no prepared program".into()),
            "Scene interaction publication",
        ))?;
        execution.validate(program, &self.state.vars, &self.state.call_stack).map_err(|message| self.eval_err(
            EvalError::InvalidFunction(message), "Scene interaction state",
        ))?;
        crate::scene_resources::validate_current_resource_state(&self.state)
            .map_err(|message| self.scene_resource_error(message))?;
        let batch = self.build_scene_batch(
            self.state.clone(), self.runtime_epoch, self.interface_epoch, self.video_epoch,
        )?;
        self.renderer.validate_scene_batch(&batch).map_err(|message| self.eval_err(
            EvalError::InvalidFunction(message), "Scene interaction renderer preflight",
        ))?;
        self.renderer.publish_scene_batch(batch);
        Ok(())
    }

    /// Retourne l'interaction actuellement pointée par le PC, sans modifier l'état.
    pub fn current_interaction(&self) -> Result<Option<Interaction>, RuntimeError> {
        if self.state.motions.waiting.is_some() || self.state.videos.waiting.is_some() {
            return Ok(None);
        }
        let Some(stmt) = self.script.get(self.state.pc) else {
            return Ok(None);
        };
        if Self::consumer_dialogue_parts(stmt).is_some(){return self.cached_consumer_dialogue().map(|data|self.retained_dialogue(data).map(Some)).unwrap_or(Ok(None));}
        if Self::consumer_choice_options(stmt).is_some(){return self.cached_consumer_choice().map(|c|self.retained_choice_labels(c).map(|options|Some(Interaction::Choice{options}))).unwrap_or(Ok(None));}
        if Self::consumer_imagemap_parts(stmt).is_some(){return self.cached_consumer_imagemap().map(|data|self.retained_imagemap(data).map(Some)).unwrap_or(Ok(None));}

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

    /// The displayed interaction can outlive the live PC after a timer/event.
    pub fn displayed_interaction(&self) -> Result<Option<Interaction>, RuntimeError> {
        if let Some(checkpoint) = self
            .state
            .scene_execution
            .as_ref()
            .and_then(|execution| execution.display.as_ref())
        {
            return self.checkpoint_interaction(checkpoint).map(Some);
        }
        self.current_interaction()
    }

    fn checkpoint_interaction(
        &self,
        checkpoint: &crate::scene::DisplayCheckpoint,
    ) -> Result<Interaction, RuntimeError> {
        let program = self.narrative.as_ref().ok_or_else(|| {
            self.eval_err(
                EvalError::InvalidFunction("A scene display requires its prepared program".into()),
                "Scene display",
            )
        })?;
        program
            .validate_position(&checkpoint.position)
            .map_err(|message| {
                self.eval_err(EvalError::InvalidFunction(message), "Scene display")
            })?;
        if let Some(data)=&checkpoint.consumer_dialogue {
            if data.pc!=checkpoint.position.pc||!matches!(checkpoint.display,HistoryDisplay::Dialogue{..}){return Err(self.scene_resource_error("Retained Dialogue differs from actual displayed visit".into()));}
            return self.retained_dialogue(data);
        }
        if let Some(data)=&checkpoint.consumer_imagemap{
            if data.pc!=checkpoint.position.pc{return Err(self.scene_resource_error("Retained Imagemap differs from its actual displayed visit".into()));}
            let interaction=self.retained_imagemap(data)?;
            if !matches!((&interaction,&checkpoint.display),(Interaction::Imagemap{background,hover_image,hotspots},HistoryDisplay::Imagemap{background:shown,hover_image:hover,hotspots:regions})if background==shown&&hover_image==hover&&hotspots==regions){return Err(self.scene_resource_error("Retained Imagemap display differs from its exact frozen definition".into()));}
            return Ok(interaction);
        }
        if let Some(choice)=&checkpoint.consumer_choice{
            if choice.pc!=checkpoint.position.pc||choice.indices()!=checkpoint.choice_indices{return Err(self.scene_resource_error("Retained Choice differs from its actual displayed visit".into()));}
            return self.retained_choice_labels(choice).map(|options|Interaction::Choice{options});
        }
        let render_text = |text: &rvn_parser::InterpolatedText| -> Result<String, RuntimeError> {
            let key = text_to_locale_key(text);
            let translated = self.translate(&key);
            let template = if translated == key {
                text.clone()
            } else {
                match rvn_parser::parse_interpolated_str(translated) {
                    Ok(template) => template,
                    Err(_) => return Ok(translated.to_owned()),
                }
            };
            self.functions
                .interpolate_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                    &template,
                    &checkpoint.variables,
                    &checkpoint.lexical_globals,
                    &mut checkpoint.random.clone(), self.locale.as_ref(),
                )
                .map_err(|error| self.eval_err(error, "Frozen scene display"))
        };
        match (self.script.get(checkpoint.position.pc), &checkpoint.display) {
            (Some(Statement::Dialogue { character_id, text }), HistoryDisplay::Dialogue { .. }) => {
                Ok(Interaction::Dialogue {
                    character: character_id.clone(),
                    text: render_text(text)?,
                })
            }
            (
                Some(Statement::Choice { options }),
                HistoryDisplay::Choice { options: displayed },
            ) => {
                if checkpoint.choice_indices.len() != displayed.len()
                    || checkpoint
                        .choice_indices
                        .iter()
                        .any(|index| *index >= options.len())
                {
                    return Err(self.eval_err(
                        EvalError::InvalidFunction("Frozen choice identity is invalid".into()),
                        "Scene display",
                    ));
                }
                let labels = checkpoint
                    .choice_indices
                    .iter()
                    .map(|index| render_text(&options[*index].label))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Interaction::Choice { options: labels })
            }
            (
                Some(Statement::Imagemap {
                    background,
                    hover_image,
                    hotspots,
                }),
                HistoryDisplay::Imagemap { .. },
            ) => Ok(Interaction::Imagemap {
                background: background.clone(),
                hover_image: hover_image.clone(),
                hotspots: hotspots.clone(),
            }),
            _ => Err(self.eval_err(
                EvalError::InvalidFunction(
                    "Frozen display does not belong to this story position".into(),
                ),
                "Scene display",
            )),
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
                    Some(statement) if Self::dialogue_statement(statement)
                ) {
                    return None;
                }
                Some(
                    self.resolve_recorded_dialogue(&crate::types::DialogueSnapshot {
                        consumer_dialogue: entry.state.last_dialogue.as_ref().and_then(|snapshot| snapshot.consumer_dialogue.clone()),
                        pc: entry.state.pc,
                        random: entry.state.display_random,
                        lexical_globals: entry
                            .state
                            .last_dialogue
                            .as_ref()
                            .and_then(|snapshot| snapshot.lexical_globals.clone()),
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
        if let Some(data)=&snapshot.consumer_dialogue {if data.pc!=snapshot.pc{return Err(self.scene_resource_error("Recorded Dialogue differs from actual source position".into()));}let Interaction::Dialogue{character,text}=self.retained_dialogue(data)? else{unreachable!()};return Ok((character,text));}
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
            .interpolate_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), 
                &template,
                &snapshot.vars,
                snapshot.lexical_globals.as_ref().unwrap_or(&snapshot.vars),
                &mut snapshot.random.clone(), self.locale.as_ref(),
            )
            .map(|text| (character_id.clone(), text))
            .map_err(|e| self.eval_err(e, "History translation"))
    }

    /// Valide un dialogue affiché et avance au statement suivant.
    pub fn advance_dialogue(&mut self) -> Result<(), RuntimeError> {
        if self.state.videos.waiting.is_some() {
            return Ok(());
        }
        if self.script.get(self.state.pc).and_then(Self::consumer_dialogue_parts).is_some()
            || self.state.scene_execution.as_ref().and_then(|e|e.display.as_ref()).is_some_and(|d|d.consumer_dialogue.is_some()) {
            return self.advance_consumer_dialogue();
        }
        if let Some(execution) = &mut self.state.scene_execution {
            let displayed_pc = execution
                .display
                .as_ref()
                .map(|display| display.position.pc);
            execution.display = None;
            if displayed_pc.is_some_and(|pc| pc != self.state.pc) {
                return Ok(());
            }
        }
        match self.script.get(self.state.pc) {
            Some(Statement::Dialogue { .. }) => {
                self.state.display_random_pc = None;
                self.state.pc += 1;
                self.sync_narrative_position();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Soumet un choix utilisateur au moteur.
    pub fn submit_choice(&mut self, selected: usize) -> Result<(), RuntimeError> {
        if self.script.get(self.state.pc).and_then(Self::consumer_choice_options).is_some(){return self.submit_consumer_choice(selected);}
        if self.state.videos.waiting.is_some() {
            return Ok(());
        }
        let Some(stmt) = self.script.get(self.state.pc).cloned() else {
            return Ok(());
        };
        let Statement::Choice { options } = stmt else {
            return Ok(());
        };
        let active = self.visible_choice_indices(&options)?;
        self.state.display_random_pc = None;

        if active.is_empty() {
            self.state.pc += 1;
            self.sync_narrative_position();
            return Ok(());
        }

        let visible = selected.min(active.len().saturating_sub(1));
        let body = options[active[visible]].body.clone();
        if !body.is_empty() {
            self.exec_silent(body[0].clone())?;
        } else {
            self.state.pc += 1;
            self.sync_narrative_position();
        }
        Ok(())
    }

    /// Soumet un hotspot d'imagemap au moteur.
    pub fn submit_hotspot(&mut self, selected: usize) -> Result<(), RuntimeError> {
        if self.script.get(self.state.pc).and_then(Self::consumer_imagemap_parts).is_some(){return self.submit_consumer_imagemap(selected);}
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
            self.sync_narrative_position();
            return Ok(());
        }

        let idx = selected.min(hotspots.len().saturating_sub(1));
        let body = hotspots[idx].body.clone();
        if !body.is_empty() {
            self.exec_silent(body[0].clone())?;
        } else {
            self.state.pc += 1;
            self.sync_narrative_position();
        }
        Ok(())
    }

    /// Soumet une sélection générique. Utile côté UI, où un clic peut venir soit
    /// d'un choice soit d'une imagemap.
    pub fn submit_selection(&mut self, selected: usize) -> Result<(), RuntimeError> {
        match Self::consumer_leaf(self.script.get(self.state.pc)) {
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
        if crate::widget_class_source_0950::AdmittedWidgetClasses::state_has_instances(&self.state){return Err(crate::save::SaveError::Incompatible("Widget class instances require their source-bound Save15 adapter".into()));}
        let snapshot=self.snapshot_for_save()?; manager.save_snapshot(&snapshot,slot,label,script_name)
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
        self.functions.reject_unit_portable_owner(&self.script).map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;
        if Self::has_widget_replacement_instances(&self.state){return self.load_historical_with_widget_retirement(data);}
        self.load_data_without_widget_retirement(data)
    }

    // Validation is the original historical loader. The replacement adapter
    // invokes this only on the existing disposable planning Engine; never on
    // the old live owner before target and retirement have both been admitted.
    fn load_data_without_widget_retirement(
        &mut self,
        data:SaveData,
    )->Result<LoadCompatibility,crate::save::SaveError>{
        use crate::save::SaveError;
        if data.ui.screens.iter().any(|instance|instance.widget_class.is_some())||data.scene_execution.as_ref().is_some_and(|execution|execution.active.iter().chain(execution.frames.iter().filter_map(|frame|frame.parked.as_ref())).any(|active|active.tasks.suspended_ui.screens.iter().any(|instance|instance.widget_class.is_some()))){return Err(SaveError::Incompatible("Historical save cannot grant Widget class provenance; use Save15".into()));}
        if self.consumer_program.has_plans() && data.format_version <= crate::save::MAX_SUPPORTED_SAVE_FORMAT_VERSION {
            return Err(SaveError::Incompatible("This frozen scheduled program requires an opt-in Save15 checkpoint; old versions cannot attach an empty or stale frame".into()));
        }
        data.validate_checkpoint_shape()?;
        if !(1..=crate::save::MAX_SUPPORTED_SAVE_FORMAT_VERSION).contains(&data.format_version) {
            return Err(SaveError::Incompatible(
                "version du format non prise en charge".into(),
            ));
        }
        let identity = function_libraries1023::story_identity1023(&self.script,&self.functions).map_err(|error|SaveError::Incompatible(error.to_string()))?;
        let compatibility = match &data.story_identity {
            Some(saved) if saved == &identity => LoadCompatibility::Verified,
            Some(_) => return Err(SaveError::Incompatible("l’histoire a changé depuis cette sauvegarde ; aucune donnée de la partie actuelle n’a été remplacée".into())),
            None => LoadCompatibility::LegacyUnchecked,
        };
        let mut data = data;
        // Save1-14 had no canonical names. Preserve the historical raw-script
        // registry for an absent/empty old field; Native programs use Save15
        // and never enter this path. No command, gather, or callback is run.
        if data.layered.display_names.is_empty() {
            data.layered.display_names = crate::composition::legacy_display_names(&self.script)
                .map_err(SaveError::Incompatible)?;
        }
        let scene_format = if data.format_version >= crate::save::TEXT_SAVE_FORMAT_VERSION {
            // Retained Text can extend a historical story or explicit scenes.
            // The continuation payload, rather than the text representation,
            // chooses the narrative restoration path.
            data.scene_execution.is_some()
        } else {
            matches!(data.format_version, crate::save::SCENE_SAVE_FORMAT_VERSION | crate::save::EXTENDED_SCENE_SAVE_FORMAT_VERSION)
        };
        if scene_format != data.scene_execution.is_some() {
            return Err(SaveError::Incompatible(
                "The save format and narrative activation disagree".into(),
            ));
        }
        if self.narrative.is_some() != scene_format {
            return Err(SaveError::Incompatible(
                "This save does not identify an explicit scene continuation; its original file has been preserved".into()));
        }
        if scene_format {
            if compatibility != LoadCompatibility::Verified {
                return Err(SaveError::Incompatible(
                    "Scene saves require a verified prepared-story identity".into(),
                ));
            }
            let mut data = data;
            if data.format_version == crate::save::SCENE_SAVE_FORMAT_VERSION {
                crate::cinematic_state::prepare_legacy_scene_save(&mut data).map_err(SaveError::Incompatible)?;
            }
            return self.load_scene_data(data, compatibility);
        }
        if data.pc > self.script.len()
            || data.call_stack.len() > 128
            || data.call_stack.iter().any(|pc| *pc > self.script.len())
            || data
                .display_random_pc
                .is_some_and(|pc| pc >= self.script.len())
            || data.last_dialogue.as_ref().is_some_and(|snapshot| {
                !matches!(
                    self.script.get(snapshot.pc),
                    Some(statement) if Self::dialogue_statement(statement)
                )
            })
        {
            return Err(SaveError::Incompatible(
                "position narrative ou pile d’appels invalide".into(),
            ));
        }
        let display_generation=self.display_generation.checked_add(1)
            .ok_or_else(||SaveError::Incompatible("Display generation exhausted".into()))?;
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
        let ui_globals = self.global_vars_for_state(&next);
        self.ui_library.with_locale(self.locale.as_ref())
            .reconcile_canvas_states(&mut next.ui, &self.functions, &ui_globals)
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
            .describe_videos(&next, epoch, self.runtime_epoch)
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        if let Some(waiting) = &next.videos.waiting {
            if !next.videos.tracks[waiting].clip.cinematic
                && !self.startup_video_wait_matches(&next)
                && !next
                    .pc
                    .checked_sub(1)
                    .and_then(|pc| self.script.get(pc))
                    .is_some_and(|statement| matches!(Self::consumer_leaf(Some(statement)), Some(Statement::VideoWait { .. })))
            {
                return Err(SaveError::Incompatible(
                    "Video wait does not match the saved narrative position".into(),
                ));
            }
        }
        if next.motions.waiting.is_some()
            && !matches!(Self::consumer_leaf(self.script.get(next.pc)), Some(Statement::MotionWait { .. }))
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
        self.restore_legacy_preview_visit(display_generation);
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

    fn load_scene_data(
        &mut self,
        data: SaveData,
        compatibility: LoadCompatibility,
    ) -> Result<LoadCompatibility, crate::save::SaveError> {
        let batch = self.scene_restore_batch(data.into_game_state())?;
        self.publish_scene_restore(batch);
        self.history.clear();
        self.renderer.loaded_compatibility(compatibility);
        Ok(compatibility)
    }

    fn scene_restore_batch(
        &self,
        mut next: GameState,
    ) -> Result<crate::renderer::SceneRenderBatch, crate::save::SaveError> {
        use crate::save::SaveError;
        // Restore preparation may describe a different UI state. Keep the live
        // source cache unchanged; the returned batch owns its descriptions.
        let _cache = self.audio_interface_cache_transaction()
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        self.consumer_program.validate_emitted(&self.script).map_err(|error|SaveError::Incompatible(error.to_string()))?;
        self.consumer_program.validate_state_binding(&next).map_err(|error|SaveError::Incompatible(error.to_string()))?;
        self.display_generation.checked_add(1).ok_or_else(|| SaveError::Incompatible("Display generation exhausted".into()))?;
        crate::display_reveal::validate_state(&next).map_err(SaveError::Incompatible)?;
        let program = self
            .narrative
            .as_ref()
            .ok_or_else(|| SaveError::Incompatible("Missing prepared scene program".into()))?;
        let execution = next
            .scene_execution
            .as_ref()
            .ok_or_else(|| SaveError::Incompatible("Missing scene activation".into()))?;
        if next.pc != execution.live_position.pc
            || next.story_identity.as_deref() != self.state.story_identity.as_deref()
        {
            return Err(SaveError::Incompatible(
                "Scene position or prepared-story identity is incoherent".into(),
            ));
        }
        execution
            .validate(program, &next.vars, &next.call_stack)
            .map_err(SaveError::Incompatible)?;
        self.validate_narrative_loop_gathers(&next,self.locale.as_ref()).map_err(|error|SaveError::Incompatible(error.to_string()))?;
        crate::scene_resources::validate_current_resource_state(&next)
            .map_err(SaveError::Incompatible)?;
        crate::audio_checkpoint::require_saved_checkpoints(&next).map_err(SaveError::Incompatible)?;
        self.renderer.validate_audio_restore(&next).map_err(SaveError::Incompatible)?;
        if next
            .display_random_pc
            .is_some_and(|pc| pc >= self.script.len())
            || next.last_dialogue.as_ref().is_some_and(|snapshot| {
                !matches!(
                    self.script.get(snapshot.pc),
                    Some(statement) if Self::dialogue_statement(statement)
                )
            })
        {
            return Err(SaveError::Incompatible(
                "Invalid recorded narrative display".into(),
            ));
        }
        if let Some(display) = &execution.display {
            if display.position.pc != next.current_interactive_pc {
                return Err(SaveError::Incompatible(
                    "The frozen display position is incoherent".into(),
                ));
            }
            for variables in [&display.variables, &display.lexical_globals] {
                crate::scene::NarrativeBudget::default()
                    .values(variables)
                    .map_err(SaveError::Incompatible)?;
            }
            self.checkpoint_interaction(display)
                .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        }
        if let Some(dialogue) = &next.last_dialogue {
            crate::scene::NarrativeBudget::default()
                .values(&dialogue.vars)
                .map_err(SaveError::Incompatible)?;
            if let Some(globals) = &dialogue.lexical_globals {
                crate::scene::NarrativeBudget::default()
                    .values(globals)
                    .map_err(SaveError::Incompatible)?;
            }
        }
        next.motions.validate().map_err(SaveError::Incompatible)?;
        next.videos.validate().map_err(SaveError::Incompatible)?;
        if next.motions.waiting.is_some()
            && !matches!(
                Self::consumer_leaf(next.pc.checked_sub(1).and_then(|pc| self.script.get(pc))),
                Some(Statement::MotionWait { .. })
            )
        {
            return Err(SaveError::Incompatible(
                "The live animation wait has no matching instruction".into(),
            ));
        }
        if let Some(alias) = &next.videos.waiting {
            let task = execution
                .resources
                .task(
                    crate::scene_resources::ResourceTaskKind::Video,
                    alias,
                    execution.owner(),
                )
                .ok_or_else(|| {
                    SaveError::Incompatible("The waited video has no live authority".into())
                })?;
            let exact_continuation = execution.resources.waits.iter().any(|wait| {
                wait.owner == execution.owner()
                    && !wait.parked
                    && wait.key == task.key
                    && wait.revision == task.revision
                    && wait.target == task.target
                    && wait.position == execution.live_position
            });
            let explicit_wait = matches!(
                Self::consumer_leaf(next.pc.checked_sub(1).and_then(|pc| self.script.get(pc))),
                Some(Statement::VideoWait { .. })
            );
            // Cinematic waits also originate in Play and UI/media callbacks.
            // Their exact saved continuation must still identify the live PC.
            if !exact_continuation || !explicit_wait && !next.videos.tracks[alias].clip.cinematic && !self.startup_video_wait_matches(&next) {
                return Err(SaveError::Incompatible(
                    "The live video wait has no matching instruction or cinematic continuation"
                        .into(),
                ));
            }
        }
        // Reconcile saved component state without firing Open/Init handlers.
        let ui_globals = self.global_vars_for_state(&next);
        self.ui_library.with_locale(self.locale.as_ref())
            .reconcile_canvas_states(&mut next.ui, &self.functions, &ui_globals)
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        if let Some(execution) = &mut next.scene_execution {
            for frame in &mut execution.frames {
                if let Some(parent) = &mut frame.parked {
                    self.ui_library.with_locale(self.locale.as_ref())
                        .reconcile_canvas_states(
                            &mut parent.tasks.suspended_ui,
                            &self.functions,
                            &ui_globals,
                        )
                        .map_err(|error| SaveError::Incompatible(error.to_string()))?;
                }
            }
        }
        crate::scene_resources::validate_current_resource_state(&next)
            .map_err(SaveError::Incompatible)?;
        next.speech_requests.clear();
        next.menu_requests.clear();
        let epoch = self
            .runtime_epoch
            .checked_add(1)
            .ok_or_else(|| SaveError::Incompatible("Runtime generation exhausted".into()))?;
        let interface_epoch = self
            .interface_epoch
            .checked_add(1)
            .ok_or_else(|| SaveError::Incompatible("Interface generation exhausted".into()))?;
        let video_epoch = self
            .video_epoch
            .checked_add(1)
            .ok_or_else(|| SaveError::Incompatible("Video generation exhausted".into()))?;
        let batch = self
            .build_scene_batch(next, epoch, interface_epoch, video_epoch)
            .map_err(|error| SaveError::Incompatible(error.to_string()))?;
        self.renderer
            .validate_scene_batch(&batch)
            .map_err(SaveError::Incompatible)?;
        Ok(batch)
    }

    fn publish_scene_restore(&mut self, batch: crate::renderer::SceneRenderBatch) {
        // scene_restore_batch preflighted this exact candidate before publication.
        self.display_generation = self.display_generation.checked_add(1).expect("validated display generation");
        self.display_owner = batch.state.scene_execution.as_ref().and_then(|execution| execution.owner());
        // Load clears history; rollback pops the restored visit. Neither may adopt
        // an earlier coincident position/owner entry as its feedback destination.
        self.display_history_entry = None;
        self.legacy_preview_visit = None;
        // Accepted Load/rollback replaces runtime Host authority, never save RNG.
        self.reset_host_random_instance();
        // A timeline replacement abandons the old stopped owner; Save15 binds
        // its separately hydrated new owner immediately after publication.
        self.restored_widget_barrier=false;
        self.runtime_epoch = batch.runtime_epoch;
        self.interface_epoch = batch.interface_epoch;
        self.video_epoch = batch.video_epoch;
        self.state = batch.state.clone();
        self.active_timer = None;
        self.input_state = Default::default();
        self.renderer.publish_scene_batch(batch);
    }

    pub fn is_finished(&self) -> bool {
        self.state.pc >= self.script.len() && self.state.videos.waiting.is_none()
    }
    pub fn get_var(&self, name: &str) -> Option<&Value> {
        self.state
            .scene_execution
            .as_ref()
            .and_then(|execution| execution.active.as_ref())
            .and_then(|active| active.locals.get(name))
            .or_else(|| self.state.vars.get(name))
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
        let views = if matches!(target, crate::motion::MotionTarget::Interface { .. }) {
            self.describe_interface_structure(state)?
        } else {
            Vec::new()
        };
        Ok(Self::motion_target_exists_in_views(target, state, &views))
    }
    fn motion_target_exists_in_views(
        target: &crate::motion::MotionTarget,
        state: &GameState,
        views: &[rvn_ui::programmable::ScreenView],
    ) -> bool {
        use crate::motion::MotionTarget;
        match target {
            MotionTarget::Background => !state.background_image.is_empty(),
            MotionTarget::Sprite { id } => {
                state.sprites.get(id).is_some_and(|sprite| sprite.visible)
            }
            MotionTarget::Layer { id, layer } => {
                state.sprites.get(id).is_some_and(|sprite| sprite.visible)
                    && state.layered.layer_visible(id, layer)
            }
            MotionTarget::Interface { screen, element } => views
                .iter()
                .find(|view| view.name == *screen)
                .is_some_and(|view| view.root.find(element).is_some()),
        }
    }
    fn cancel_absent_motions(
        &self,
        state: &GameState,
        motions: &mut crate::motion::MotionState,
    ) -> Result<(), RuntimeError> {
        let views = if motions
            .tracks
            .values()
            .any(|track| matches!(track.target, crate::motion::MotionTarget::Interface { .. }))
        {
            self.describe_interface_structure(state)?
        } else {
            Vec::new()
        };
        let mut absent = Vec::new();
        for (key, track) in &motions.tracks {
            let exists = if state.scene_execution.is_some()
                && matches!(track.target, crate::motion::MotionTarget::Interface { .. })
            {
                self.motion_projection_binding(state, key, track)?.is_some()
            } else {
                Self::motion_target_exists_in_views(&track.target, state, &views)
            };
            if !exists {
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
        let mut views = state
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
        if state.scene_execution.is_some() {
            for view in &mut views {
                if matches!(view.target, crate::motion::MotionTarget::Interface { .. }) {
                    let key = view.target.key();
                    let track = &state.motions.tracks[&key];
                    view.ui_projection = self.motion_projection_binding(state, &key, track)?;
                    if view.ui_projection.is_none() {
                        return Err(self.scene_resource_error(
                            "Motion interface component is no longer resident".into(),
                        ));
                    }
                }
            }
        }
        Ok(views)
    }

    fn projection_has_component(
        &self,
        state: &GameState,
        binding: &crate::ui_residency::UiProjectionBinding,
    ) -> Result<bool, RuntimeError> {
        let resident = crate::ui_residency::resident_interface(state, &binding.interface)
            .map_err(|message| self.scene_resource_error(message))?;
        if resident.instance.order != binding.order
            || resident.instance.host_role != binding.host_role
            || resident.residence != binding.residence
        {
            return Err(self.scene_resource_error(
                "Interface projection no longer addresses its captured instance".into(),
            ));
        }
        // A parked instance is resident but has no mounted tree to evaluate.
        // Return validates its visible structure before publishing the remount.
        if resident.residence == crate::ui_residency::ProjectionResidence::Parked {
            return Ok(true);
        }
        let ui = crate::ui::UiState {
            screens: vec![resident.instance.clone()],
            next_order: state.ui.next_order,
            host_next_order: state.ui.host_next_order,
            instance_registry:None,
        };
        let views = self
            .ui_library.with_locale(self.locale.as_ref())
            .structural_views(&ui, &self.functions, &self.global_vars_for_state(state))
            .map_err(|error| self.eval_err(error, "Resident interface structure"))?;
        Ok(views
            .iter()
            .any(|view| view.root.find(&binding.element).is_some()))
    }

    fn motion_projection_binding(
        &self,
        state: &GameState,
        key: &str,
        track: &crate::motion::Track,
    ) -> Result<Option<crate::ui_residency::UiProjectionBinding>, RuntimeError> {
        let execution = state.scene_execution.as_ref().ok_or_else(|| {
            self.scene_resource_error("Narrative motion residency is unavailable".into())
        })?;
        let task = execution
            .resources
            .task(
                crate::scene_resources::ResourceTaskKind::Motion,
                key,
                execution.owner(),
            )
            .ok_or_else(|| {
                self.scene_resource_error("Motion has no live resource authority".into())
            })?;
        let binding = crate::ui_residency::motion_binding_parts(
            &state.ui,
            &execution.resources,
            execution.active.as_ref(),
            &execution.frames,
            task,
            track,
        )
        .map_err(|message| self.scene_resource_error(message))?;
        match binding {
            Some(binding) if self.projection_has_component(state, &binding)? => Ok(Some(binding)),
            _ => Ok(None),
        }
    }

    pub(crate) fn video_projection_binding(
        &self,
        state: &GameState,
        id: &str,
        track: &crate::video::Track,
    ) -> Result<Option<crate::ui_residency::UiProjectionBinding>, RuntimeError> {
        if track.clip.target.is_none() || state.scene_execution.is_none() {
            return Ok(None);
        }
        let execution = state.scene_execution.as_ref().unwrap();
        let task = execution
            .resources
            .task(
                crate::scene_resources::ResourceTaskKind::Video,
                id,
                execution.owner(),
            )
            .ok_or_else(|| {
                self.scene_resource_error("Video has no live resource authority".into())
            })?;
        let binding = crate::ui_residency::interface_projection(state, &task.key)
            .map_err(|message| self.scene_resource_error(message))?
            .ok_or_else(|| {
                self.scene_resource_error("Embedded player has no captured interface".into())
            })?;
        if self.projection_has_component(state, &binding)? {
            Ok(Some(binding))
        } else {
            Ok(None)
        }
    }

    fn normalize_resident_projection_targets(
        &self,
        state: &mut GameState,
    ) -> Result<bool, RuntimeError> {
        let before_motion_count = state.motions.tracks.len();
        let mut motions = std::mem::take(&mut state.motions);
        self.cancel_absent_motions(state, &mut motions)?;
        state.motions = motions;
        state
            .videos
            .validate()
            .map_err(|error| self.eval_err(EvalError::InvalidFunction(error), "Video state"))?;
        let legacy_views = if state.scene_execution.is_none() {
            self.describe_interface_structure(state)?
        } else {
            Vec::new()
        };
        let mut absent = Vec::new();
        for (id, track) in &state.videos.tracks {
            let exists = if track.clip.target.is_none() {
                true
            } else if state.scene_execution.is_some() {
                self.video_projection_binding(state, id, track)?.is_some()
            } else {
                let (screen, element) = track
                    .clip
                    .target
                    .as_ref()
                    .unwrap()
                    .strip_prefix("ui:")
                    .unwrap()
                    .split_once('/')
                    .unwrap();
                legacy_views
                    .iter()
                    .any(|view| view.name == screen && view.root.find(element).is_some())
            };
            if !exists {
                absent.push(id.clone());
            }
        }
        let changed = before_motion_count != state.motions.tracks.len() || !absent.is_empty();
        for id in absent {
            state.videos.tracks.remove(&id);
        }
        if state
            .videos
            .waiting
            .as_ref()
            .is_some_and(|id| !state.videos.tracks.contains_key(id))
        {
            state.videos.waiting = None;
        }
        Ok(changed)
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
            let wait_pc = if self.narrative.is_some() {
                pc.checked_sub(1)
            } else {
                Some(pc)
            };
            if !matches!(
                Self::consumer_leaf(wait_pc.and_then(|pc| self.script.get(pc))),
                Some(Statement::MotionWait { .. })
            ) {
                return Err(self.eval_err(
                    EvalError::InvalidFunction(
                        "Animation wait does not match the narrative position".into(),
                    ),
                    "Animation clock",
                ));
            }
            candidate.waiting = None;
            if self.narrative.is_none() {
                pc += 1;
            }
        }
        if self.narrative.is_some() {
            let mut next = self.state.clone();
            next.motions = candidate;
            let completed: Vec<_> = next
                .motions
                .tracks
                .iter()
                .filter(|(_, track)| !track.running)
                .filter_map(|(alias, _)| {
                    let execution = next.scene_execution.as_ref()?;
                    let task = execution.resources.task(
                        crate::scene_resources::ResourceTaskKind::Motion,
                        alias,
                        execution.owner(),
                    )?;
                    execution.resources.effect_token(
                        self.runtime_epoch,
                        &task.key,
                        execution.owner(),
                    )
                })
                .collect();
            for token in completed {
                crate::scene_resources::release_waiters(&mut next, &token, self.runtime_epoch)
                    .map_err(|message| self.scene_resource_error(message))?;
            }
            return self.commit_scene_state(next);
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

    pub fn source_menu_arity(&self, name: &str) -> Option<usize> {
        self.ui_library.screen_arity(name)
    }
    pub fn source_menu_labels(&self) -> Vec<String> {
        self.label_table.keys().cloned().collect()
    }
    /// Recovery removes only host instances of the failed presenter role.
    /// Its failed lifecycle transaction was already rolled back by the caller.
    pub fn abandon_source_menu(&mut self, role: rvn_ui::PageRole) -> Result<(), RuntimeError> {
        if !self
            .state
            .ui
            .screens
            .iter()
            .any(|screen| screen.host_role == Some(role))
        {
            return Ok(());
        }
        let mut next = self.state.clone();
        next.ui
            .screens
            .retain(|screen| screen.host_role != Some(role));
        self.commit_ui_state(next)
    }
    /// Compensate a failed external menu operation before any new input runs.
    pub fn cancel_source_menu_transaction(
        &mut self,
        previous: GameState,
    ) -> Result<(), RuntimeError> {
        self.commit_ui_state(previous)
    }
    pub fn finish_source_menu_transaction(&mut self, previous: GameState) {
        if previous.pc == self.state.pc
            && previous.story_identity == self.state.story_identity
            && (previous.vars != self.state.vars
                || previous.random != self.state.random
                || previous.ui.story_only() != self.state.ui.story_only())
        {
            let previous=if self.narrative.is_some(){crate::audio_checkpoint::unmeasured_historical_state(previous)}else{previous};
            self.history.push(previous, None);
        }
    }

    /// Host presenters reuse source evaluation and bounded lifecycle handlers.
    /// Updating their context preserves local controls, focus and canvas state.
    pub fn synchronize_source_menu(
        &mut self,
        role: rvn_ui::PageRole,
        name: Option<&str>,
        context: Value,
        modal: bool,
        layer: i32,
    ) -> Result<bool, RuntimeError> {
        self.with_host_random_transaction(|engine| engine.synchronize_source_menu_host_planned(role, name, context, modal, layer))
    }

    fn synchronize_source_menu_host_planned(
        &mut self,
        role: rvn_ui::PageRole,
        name: Option<&str>,
        context: Value,
        modal: bool,
        layer: i32,
    ) -> Result<bool, RuntimeError> {
        if name.is_none()
            && !self
                .state
                .ui
                .screens
                .iter()
                .any(|screen| screen.host_role == Some(role))
        {
            return Ok(false);
        }
        if let Some(name) = name {
            if let Some(screen) = self.state.ui.screens.iter().find(|screen| {
                screen.name == name && screen.host_role == Some(role) && screen.host_root
            }) {
                let arguments = if self.ui_library.screen_arity(name) == Some(1) {
                    vec![context.clone()]
                } else {
                    vec![]
                };
                if screen.arguments == arguments && screen.modal == modal && screen.layer == layer {
                    return Ok(true);
                }
            }
        }
        let mut next = self.state.clone();
        if let Some(name) = name {
            let arity = self.ui_library.screen_arity(name).ok_or_else(|| {
                self.eval_err(
                    EvalError::InvalidFunction(format!("Source menu screen is absent: {name}")),
                    "Source menu",
                )
            })?;
            if arity > 1 {
                return Err(self.eval_err(
                    EvalError::InvalidFunction(format!(
                        "Source menu {name} needs zero arguments or one context argument"
                    )),
                    "Source menu",
                ));
            }
            if next
                .ui
                .screens
                .iter()
                .any(|screen| screen.name == name && screen.host_role != Some(role))
            {
                return Err(self.eval_err(
                    EvalError::InvalidFunction(format!(
                        "Source menu {name} is already owned by another screen or menu role"
                    )),
                    "Source menu",
                ));
            }
        }
        let root_unchanged = next.ui.screens.iter().any(|screen| {
            screen.host_role == Some(role) && screen.host_root && name == Some(screen.name.as_str())
        });
        let closing: Vec<_> = next
            .ui
            .screens
            .iter()
            .filter(|screen| screen.host_role == Some(role) && !root_unchanged)
            .map(|screen| screen.name.clone())
            .collect();
        let mut changed = false;
        for name in closing {
            self.apply_ui_commands(&mut next, vec![crate::eval::UiCommand::Close { name }])?;
            changed = true;
        }
        if let Some(name) = name {
            let arguments = if self.ui_library.screen_arity(name) == Some(1) {
                vec![context]
            } else {
                vec![]
            };
            if let Some(screen) = next
                .ui
                .screens
                .iter_mut()
                .find(|screen| screen.name == name)
            {
                if screen.arguments != arguments || screen.modal != modal || screen.layer != layer {
                    screen.arguments = arguments;
                    screen.modal = modal;
                    screen.layer = layer;
                    changed = true;
                }
            } else {
                self.apply_ui_commands(
                    &mut next,
                    vec![crate::eval::UiCommand::Open {
                        name: name.into(),
                        arguments,
                        modal,
                        layer,
                        host_role: Some(role),
                        inherit_host: true,
                    }],
                )?;
                if let Some(screen) = next
                    .ui
                    .screens
                    .iter_mut()
                    .find(|screen| screen.name == name)
                {
                    screen.host_root = true;
                }
                changed = true;
            }
        }
        if changed {
            self.commit_ui_state(next)?;
        }
        Ok(name.is_some())
    }

    fn describe_interfaces(
        &self,
        state: &GameState,
    ) -> Result<Vec<rvn_ui::programmable::ScreenView>, RuntimeError> {
        // Trace-visible computations must not depend on projection cache heat.
        let trace_active = self.functions.debug_probe.is_some();
        let cached = if trace_active { None } else { self.cached_interface_views(state) };
        let views = if let Some(views) = cached {
            views
        } else {
            let views = self.build_interface_views(state)?;
            if !trace_active {
                let mut cache = self
                    .interface_view_cache
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                if cache.len() == 2 {
                    cache.remove(0);
                }
                cache.push(InterfaceViewSnapshot {
                    locale: self.locale.clone(),
                    text_resources: self.functions.text_resources().revision_stamp(),
                    ui: state.ui.clone(),
                    globals: self.global_vars_for_state(state),
                    views: views.clone(),
                });
            }
            views
        };
        self.translate_interface_views(state, views)
    }

    // Portable input validation must observe real widgets without changing the
    // cache that is part of the suspended command's exact Engine source.
    fn describe_uncached_interfaces(
        &self,
        state: &GameState,
    ) -> Result<Vec<rvn_ui::programmable::ScreenView>, RuntimeError> {
        self.translate_interface_views(state, self.build_interface_views(state)?)
    }

    fn build_interface_views(
        &self,
        state: &GameState,
    ) -> Result<Vec<rvn_ui::programmable::ScreenView>, RuntimeError> {
        #[cfg(test)]
        self.interface_view_builds
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.ui_library.with_locale(self.locale.as_ref())
            .views(&state.ui, &self.functions, &self.global_vars_for_state(state))
            .map_err(|error| self.eval_err(error, "Interface"))
    }

    fn cached_interface_views(
        &self,
        state: &GameState,
    ) -> Option<Vec<rvn_ui::programmable::ScreenView>> {
        let mut cache = self
            .interface_view_cache
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let globals = self.global_vars_for_state(state);
        let text_resources = self.functions.text_resources().revision_stamp();
        let index = cache.iter().rposition(|entry| entry.matches(state, &globals, self.locale.as_ref(), &text_resources))?;
        // Recovery reads the previous state after describing the candidate.
        // Promote hits so that the next frame's current snapshot is retained
        // when its candidate arrives, rather than redrawing it for recovery.
        let entry = cache.remove(index);
        let views = entry.views.clone();
        cache.push(entry);
        Some(views)
    }

    fn describe_interface_structure(
        &self,
        state: &GameState,
    ) -> Result<Vec<rvn_ui::programmable::ScreenView>, RuntimeError> {
        // Structure-only preparation must discover the same genuine roots.
        let cached = if self.functions.debug_probe.is_some() { None } else { self.cached_interface_views(state) };
        let views = if let Some(mut views) = cached {
            for view in &mut views {
                view.root
                    .visit_mut(&mut |component| component.drawing = None);
            }
            views
        } else {
            self.ui_library.with_locale(self.locale.as_ref())
                .structural_views(&state.ui, &self.functions, &self.global_vars_for_state(state))
                .map_err(|error| self.eval_err(error, "Interface"))?
        };
        self.translate_interface_views(state, views)
    }

    fn translate_interface_views(
        &self,
        state: &GameState,
        mut views: Vec<rvn_ui::programmable::ScreenView>,
    ) -> Result<Vec<rvn_ui::programmable::ScreenView>, RuntimeError> {
        let globals = self.global_vars_for_state(state);
        let widget_objects=self.widget_object_context(state)?;
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
                            self.functions.interpolate_with_locale_with_widget_objects(widget_objects.as_ref(), 
                                &template,
                                &globals,
                                &mut random, self.locale.as_ref(),
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

    pub fn interface_hosts(&self)->Result<Vec<rvn_ui::programmable::ScreenHostView>,RuntimeError>{
        let mut hosts=self.ui_library.with_locale(self.locale.as_ref()).host_views(&self.state.ui,&self.functions,&self.global_vars_for_state(&self.state)).map_err(|e|self.eval_err(e,"UserWidget owner description"))?;
        let children=self.translate_interface_views(&self.state,hosts.iter_mut().filter_map(|host|host.child.take()).collect())?;
        for host in &mut hosts{host.child=children.iter().find(|child|child.name==host.name).cloned();}Ok(hosts)
    }
    pub fn interface_is_modal(&self) -> bool {
        self.state.ui.screens.iter().any(|screen| screen.modal)
    }

    fn describe_videos(
        &self,
        state: &GameState,
        epoch: u64,
        runtime_epoch: u64,
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
                let ui_projection = self.video_projection_binding(state, id, track)?;
                if let Some(target) = &track.clip.target {
                    let (screen, element) =
                        target.strip_prefix("ui:").unwrap().split_once('/').unwrap();
                    let exists = if state.scene_execution.is_some() {
                        ui_projection.is_some()
                    } else {
                        interfaces
                            .iter()
                            .any(|view| view.name == screen && view.root.find(element).is_some())
                    };
                    if !exists {
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
                let resource_token = if let Some(execution) = &state.scene_execution {
                    let task = execution
                        .resources
                        .task(
                            crate::scene_resources::ResourceTaskKind::Video,
                            id,
                            execution.owner(),
                        )
                        .ok_or_else(|| {
                            self.scene_resource_error(
                                "Video player has no live resource authority".into(),
                            )
                        })?;
                    Some(
                        execution
                            .resources
                            .effect_token(runtime_epoch, &task.key, execution.owner())
                            .ok_or_else(|| {
                                self.scene_resource_error(
                                    "Video player authority is suspended".into(),
                                )
                            })?,
                    )
                } else {
                    None
                };
                Ok(crate::video::VideoView {
                    id: id.clone(),
                    epoch: if resource_token.is_some() {
                        runtime_epoch
                    } else {
                        epoch
                    },
                    resource_token,
                    ui_projection,
                    track: track.clone(),
                    subtitle,
                })
            })
            .collect()
    }
    pub fn video_views(&self) -> Result<Vec<crate::video::VideoView>, RuntimeError> {
        self.describe_videos(&self.state, self.video_epoch, self.runtime_epoch)
    }
    /// Host controls use the same transactional path as RVN event handlers.
    pub fn skip_video(&mut self, id: &str) -> Result<(), RuntimeError> {
        self.with_host_random_transaction(|engine| engine.skip_video_host_planned(id))
    }

    fn skip_video_host_planned(&mut self, id: &str) -> Result<(), RuntimeError> {
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
    fn video_view_current(&self, view: &crate::video::VideoView) -> bool {
        if let Some(token) = &view.resource_token {
            view.id == token.target.alias && self.video_task_current(token)
        } else {
            self.narrative.is_none()
                && view.epoch == self.video_epoch
                && self.state.videos.tracks.contains_key(&view.id)
        }
    }
    fn video_task_current(&self, token: &crate::scene_resources::ResourceToken) -> bool {
        self.state
            .scene_execution
            .as_ref()
            .is_some_and(|execution| {
                execution
                    .resources
                    .accepts(token, self.runtime_epoch, execution.owner())
                    && execution.resources.tasks.iter().any(|task| {
                        task.key == token.task.key
                            && task.kind == crate::scene_resources::ResourceTaskKind::Video
                    })
                    && self.state.videos.tracks.contains_key(&token.target.alias)
            })
    }
    pub fn resume_video_from_view(
        &mut self,
        view: &crate::video::VideoView,
    ) -> Result<bool, RuntimeError> {
        if !self.video_view_current(view) {
            return Ok(false);
        }
        self.resume_video(&view.id)?;
        Ok(true)
    }
    pub fn skip_video_from_view(
        &mut self,
        view: &crate::video::VideoView,
    ) -> Result<bool, RuntimeError> {
        if !self.video_view_current(view) || self.state.videos.tracks[&view.id].finished() {
            return Ok(false);
        }
        self.skip_video(&view.id)?;
        Ok(true)
    }
    /// Resolve authority before inspecting an untrusted decoder payload.
    pub fn video_feedback_from_view(
        &mut self,
        view: &crate::video::VideoView,
        feedback: crate::video::Feedback,
    ) -> Result<bool, RuntimeError> {
        if let Some(token) = &view.resource_token {
            if view.id != token.target.alias {
                return Ok(false);
            }
            self.video_feedback_for_task(token, feedback)
        } else {
            self.video_feedback(view.epoch, &view.id, feedback)
        }
    }
    pub fn video_feedback_for_task(
        &mut self,
        token: &crate::scene_resources::ResourceToken,
        feedback: crate::video::Feedback,
    ) -> Result<bool, RuntimeError> {
        self.with_host_random_transaction(|engine| engine.video_feedback_for_task_host_planned(token, feedback))
    }

    fn video_feedback_for_task_host_planned(
        &mut self,
        token: &crate::scene_resources::ResourceToken,
        feedback: crate::video::Feedback,
    ) -> Result<bool, RuntimeError> {
        if !self.video_task_current(token) {
            return Ok(false);
        }
        let id = &token.target.alias;
        let track = &self.state.videos.tracks[id];
        let position = matches!(feedback, crate::video::Feedback::Position { .. });
        if track.finished()
            || (position || matches!(feedback, crate::video::Feedback::End))
                && track.playback != crate::video::Playback::Playing
        {
            return Ok(false);
        }
        let mut next = self.state.clone();
        let handler = next.videos.feedback(id, feedback).map_err(|message| {
            self.eval_err(
                EvalError::InvalidFunction(message),
                "Video renderer feedback",
            )
        })?;
        if next.videos.tracks[id].finished() {
            crate::scene_resources::release_waiters(&mut next, token, self.runtime_epoch)
                .map_err(|message| self.scene_resource_error(message))?;
        }
        if position {
            crate::scene_resources::validate_current_resource_state(&next)
                .map_err(|message| self.scene_resource_error(message))?;
            self.state.videos = next.videos;
            return Ok(true);
        }
        if let Some(handler) = handler {
            let commands = self.video_handler(&mut next, id, &handler)?;
            self.apply_callback_commands_budgeted(&mut next, commands, &mut crate::ui::CanvasBudget::default())?;
        }
        self.commit_ui_state(next)?;
        Ok(true)
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
    ) -> Result<host_random::CallbackCommands, RuntimeError> {
        self.video_handler_budgeted(next, id, handler, &mut crate::ui::CanvasBudget::default())
    }
    fn video_handler_budgeted(
        &self,
        next: &mut GameState,
        id: &str,
        handler: &str,
        budget: &mut crate::ui::CanvasBudget,
    ) -> Result<host_random::CallbackCommands, RuntimeError> {
        self.media_handler_owned_budgeted(next, id, handler, budget)
    }
    /// Position reports don't add history frames or request a new seek. Other
    /// feedback is transactional and invalidates old callbacks on completion.
    pub fn video_feedback(
        &mut self,
        epoch: u64,
        id: &str,
        feedback: crate::video::Feedback,
    ) -> Result<bool, RuntimeError> {
        self.with_host_random_transaction(|engine| engine.video_feedback_host_planned(epoch, id, feedback))
    }

    fn video_feedback_host_planned(
        &mut self,
        epoch: u64,
        id: &str,
        feedback: crate::video::Feedback,
    ) -> Result<bool, RuntimeError> {
        if self.narrative.is_some()
            || epoch != self.video_epoch
            || !self.state.videos.tracks.contains_key(id)
            || self.state.videos.tracks[id].finished()
        {
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
            self.apply_callback_commands_budgeted(&mut next, commands, &mut crate::ui::CanvasBudget::default())?;
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

    fn command_owner(state: &GameState) -> crate::scene_tasks::TaskOwner {
        state
            .scene_execution
            .as_ref()
            .and_then(|execution| execution.owner())
            .map(|activation| crate::scene_tasks::TaskOwner::Scene { activation })
            .unwrap_or(crate::scene_tasks::TaskOwner::Game)
    }
    fn live_resource(
        state: &GameState,
        kind: crate::scene_resources::ResourceTaskKind,
        alias: &str,
    ) -> Option<crate::scene_resources::ResourceTask> {
        if let Some(execution)=&state.scene_execution{return execution.resources.task(kind,alias,execution.owner()).cloned();}
        if kind==crate::scene_resources::ResourceTaskKind::Interface{return state.ui.instance_registry.as_ref()?.task(kind,alias,None).cloned();}None
    }
    fn install_owned_resource(
        &self,
        state: &mut GameState,
        owner: crate::scene_tasks::TaskOwner,
        kind: crate::scene_resources::ResourceTaskKind,
        target_kind: crate::scene_resources::TargetKind,
        alias: &str,
    ) -> Result<(), RuntimeError> {
        if let Some(execution) = &mut state.scene_execution {
            let target = execution
                .resources
                .ensure_target(owner.clone(), target_kind, alias)
                .map_err(|message| self.scene_resource_error(message))?;
            execution
                .resources
                .register(
                    crate::scene_resources::resource_key(owner, kind, alias),
                    target,
                    kind,
                    None,
                )
                .map_err(|message| self.scene_resource_error(message))?;
        }else if kind==crate::scene_resources::ResourceTaskKind::Interface{
            if let Some(registry)=&mut state.ui.instance_registry{let target=registry.ensure_target(owner.clone(),target_kind,alias).map_err(|s|self.scene_resource_error(s))?;registry.register(crate::scene_resources::resource_key(owner,kind,alias),target,kind,None).map_err(|s|self.scene_resource_error(s))?;}
        }
        Ok(())
    }
    fn renew_video_authority(
        &self,
        state: &mut GameState,
        alias: &str,
    ) -> Result<(), RuntimeError> {
        if let Some(task) = Self::live_resource(
            state,
            crate::scene_resources::ResourceTaskKind::Video,
            alias,
        ) {
            state
                .scene_execution
                .as_mut()
                .unwrap()
                .resources
                .renew_task(&task.key)
                .map_err(|message| self.scene_resource_error(message))?;
        }
        Ok(())
    }
    fn apply_ui_commands_owned(
        &self,
        next: &mut GameState,
        commands: Vec<crate::eval::UiCommand>,
        owner: crate::scene_tasks::TaskOwner,
    ) -> Result<(), RuntimeError> {
        self.apply_ui_commands_budgeted_owned(
            next,
            commands,
            &mut crate::ui::CanvasBudget::default(),
            owner,
        )
    }

    fn apply_ui_commands_budgeted(
        &self,
        next: &mut GameState,
        commands: Vec<crate::eval::UiCommand>,
        budget: &mut crate::ui::CanvasBudget,
    ) -> Result<(), RuntimeError> {
        self.apply_ui_commands_budgeted_owned(next, commands, budget, Self::command_owner(next))
    }
    fn apply_ui_commands_budgeted_owned(
        &self,
        next: &mut GameState,
        commands: Vec<crate::eval::UiCommand>,
        budget: &mut crate::ui::CanvasBudget,
        owner: crate::scene_tasks::TaskOwner,
    ) -> Result<(), RuntimeError> {
        self.apply_ui_commands_budgeted_context(next, commands, budget, owner, None)
    }

    fn apply_ui_commands_budgeted_context(
        &self,
        next: &mut GameState,
        commands: Vec<crate::eval::UiCommand>,
        budget: &mut crate::ui::CanvasBudget,
        owner: crate::scene_tasks::TaskOwner,
        random_context: Option<host_random::SharedUiRandom>,
    ) -> Result<(), RuntimeError> {
        use crate::scene_resources::{ResourceTaskKind, TargetKind};
        let mut queue: std::collections::VecDeque<_> = commands
            .into_iter()
            .map(|command| (command, owner.clone(), random_context.clone()))
            .collect();
        let native_targets=next.ui.screens.iter().filter_map(|instance|instance.consumer_projection.as_ref().map(|projection|projection.owner.target.clone())).collect::<Vec<_>>();
        let mut count = 0;
        let mut widget_retirements=std::collections::VecDeque::new();
        loop {
            let(command,owner,random_context)=if let Some(command)=queue.pop_front(){command}else if let Some(retirement)=widget_retirements.pop_front(){
                count+=1;if count>128{return Err(self.eval_err(EvalError::ExecutionLimit{limit:"128 interface commands and real Widget lifecycle events"},"Widget lifecycle"));}
                if let Some(callback)=self.finish_command_widget_retirement(next,retirement,budget)?{queue.extend(callback.commands.into_iter().map(|command|(command,callback.owner.clone(),callback.random.clone())));}
                continue;
            }else{break;};
            if next.scene_execution.is_some() {
                crate::scene_resources::sync_current(next)
                    .map_err(|message| self.scene_resource_error(message))?;
            }
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
                crate::eval::UiCommand::WidgetRemoveFromParent { object } => {
                    self.stage_callback_widget_removal(next, object)?;
                    continue;
                }
                crate::eval::UiCommand::Menu { request, source } => {
                    if !next.menu_requests.is_empty() {
                        return Err(self.eval_err(
                            EvalError::InvalidMenuRequest(
                                "Only one game-menu request is allowed per event".into(),
                            ),
                            "Menu transaction",
                        ));
                    }
                    let screen = source.as_ref().ok_or_else(|| {
                        self.eval_err(
                            EvalError::InvalidMenuRequest(
                                "Menu request lacks trusted screen origin".into(),
                            ),
                            "Menu transaction",
                        )
                    })?;
                    let instance = next
                        .ui
                        .screens
                        .iter()
                        .find(|instance| instance.name == *screen)
                        .ok_or_else(|| {
                            self.eval_err(
                                EvalError::InvalidMenuRequest(
                                    "Menu request came from an inactive screen".into(),
                                ),
                                "Menu transaction",
                            )
                        })?;
                    next.menu_requests.push(rvn_ui::source_menus::MenuEffect {
                        screen: screen.clone(),
                        screen_order: instance.order,
                        host_role: instance.host_role,
                        request: request.clone(),
                    });
                    continue;
                }
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
                    let request=rvn_ui::accessibility::SpeechRequest::Speak(spoken);
                    if self.collect_retirement_speech(next,&request)?{continue;}
                    next.speech_requests.push(request);
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
                    let request=rvn_ui::accessibility::SpeechRequest::Stop;
                    if self.collect_retirement_speech(next,&request)?{continue;}
                    next.speech_requests.push(request);
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
                    if let Some(task) = Self::live_resource(next, ResourceTaskKind::Video, name) {
                        crate::scene_resources::remove_target(
                            next,
                            &task.key.owner,
                            TargetKind::Video,
                            name,
                        )
                        .map_err(|message| self.scene_resource_error(message))?;
                    }
                    next.videos
                        .play(name.clone(), definition.clone())
                        .map_err(|message| {
                            self.eval_err(EvalError::InvalidFunction(message), "Video play")
                        })?;
                    self.install_owned_resource(
                        next,
                        owner.clone(),
                        ResourceTaskKind::Video,
                        TargetKind::Video,
                        name,
                    )?;
                    if let Some(target) = &definition.target {
                        let (screen, element) =
                            target.strip_prefix("ui:").unwrap().split_once('/').unwrap();
                        if !self
                            .describe_interface_structure(next)?
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
                        if next.scene_execution.is_some() {
                            let key = crate::scene_resources::resource_key(
                                owner.clone(),
                                ResourceTaskKind::Video,
                                name,
                            );
                            crate::ui_residency::bind_video_interface(next, &key)
                                .map_err(|message| self.scene_resource_error(message))?;
                        }
                    }
                    if definition.cinematic {
                        next.videos.wait(name).map_err(|message| {
                            self.eval_err(EvalError::InvalidFunction(message), "Video cinematic")
                        })?;
                    }
                    self.record_startup_video_command(&command)?;
                    continue;
                }
                crate::eval::UiCommand::VideoPause { name } => {
                    next.videos.pause(name).map_err(|message| {
                        self.eval_err(EvalError::InvalidFunction(message), "Video pause")
                    })?;
                    self.renew_video_authority(next, name)?;
                    self.record_startup_video_command(&command)?;
                    continue;
                }
                crate::eval::UiCommand::VideoResume { name } => {
                    next.videos.resume(name).map_err(|message| {
                        self.eval_err(EvalError::InvalidFunction(message), "Video resume")
                    })?;
                    self.renew_video_authority(next, name)?;
                    self.record_startup_video_command(&command)?;
                    continue;
                }
                crate::eval::UiCommand::VideoStop { name } => {
                    if let Some(task) = Self::live_resource(next, ResourceTaskKind::Video, name) {
                        crate::scene_resources::remove_target(
                            next,
                            &task.key.owner,
                            TargetKind::Video,
                            name,
                        )
                        .map_err(|message| self.scene_resource_error(message))?;
                    }
                    next.videos.stop(name);
                    self.record_startup_video_command(&command)?;
                    continue;
                }
                crate::eval::UiCommand::VideoSeek { name, seconds } => {
                    next.videos.seek(name, *seconds).map_err(|message| {
                        self.eval_err(EvalError::InvalidFunction(message), "Video seek")
                    })?;
                    self.renew_video_authority(next, name)?;
                    self.record_startup_video_command(&command)?;
                    continue;
                }
                crate::eval::UiCommand::VideoVolume { name, volume } => {
                    next.videos.volume(name, *volume).map_err(|message| {
                        self.eval_err(EvalError::InvalidFunction(message), "Video volume")
                    })?;
                    self.record_startup_video_command(&command)?;
                    continue;
                }
                crate::eval::UiCommand::VideoSkip { name } => {
                    let callback_task = Self::live_resource(next, ResourceTaskKind::Video, name);
                    let callback_owner = callback_task
                        .as_ref()
                        .map(|task| task.key.owner.clone())
                        .unwrap_or_else(|| owner.clone());
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
                    let handler = next
                        .videos
                        .feedback(name, crate::video::Feedback::End)
                        .map_err(|message| {
                            self.eval_err(EvalError::InvalidFunction(message), "Video skip")
                        })?;
                    if let Some(task) = callback_task {
                        let execution = next.scene_execution.as_ref().unwrap();
                        let token = execution
                            .resources
                            .effect_token(self.runtime_epoch, &task.key, execution.owner())
                            .ok_or_else(|| {
                                self.scene_resource_error("Video skip authority disappeared".into())
                            })?;
                        crate::scene_resources::release_waiters(next, &token, self.runtime_epoch)
                            .map_err(|message| self.scene_resource_error(message))?;
                    }
                    if let Some(handler) = handler {
                        let callback=self.video_handler_budgeted(next,name,&handler,budget)?;
                        queue.extend(callback.commands.into_iter().map(|command|
                            (command, callback.owner.clone(), callback.random.clone())));
                    }
                    self.record_startup_video_command(&command)?;
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
                    let selected = self.evaluate_command_random(next, &owner,
                        random_context.as_ref(), |next, random| {
                            crate::composition::resolve_attributes_budgeted(definition,
                                &rvn_ui::composition::Attributes::new(), &definition.defaults,
                                &self.functions, &self.global_vars_for_state(next), random, budget)
                                .map_err(|error| self.eval_err(error, "Character attribute selector"))
                        })?;
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
                    let definition=composed.definition.clone();
                    let current_attributes=composed.attributes.clone();
                    let selected=self.evaluate_command_random(next, &owner,
                        random_context.as_ref(), |next, random| {
                            crate::composition::resolve_attributes_budgeted(&definition,
                                &current_attributes, attributes, &self.functions, &self.global_vars_for_state(next), random, budget)
                                .map_err(|error|self.eval_err(error,"Character attribute selector"))
                        })?;
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
                                .describe_interface_structure(next)?
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
                    let old_track = next.motions.tracks.get(&target.key()).cloned();
                    if let Some(task) =
                        Self::live_resource(next, ResourceTaskKind::Motion, &target.key())
                    {
                        crate::scene_resources::supersede_task(next, &task.key)
                            .map_err(|message| self.scene_resource_error(message))?;
                        if let Some(track) = old_track {
                            next.motions.tracks.insert(target.key(), track);
                        }
                    }
                    next.motions
                        .play(target.clone(), definition.clone())
                        .map_err(|message| {
                            self.eval_err(EvalError::InvalidFunction(message), "Motion play")
                        })?;
                    if let Some(execution) = &mut next.scene_execution {
                        let (target_owner, target_kind, alias) = match target {
                            crate::motion::MotionTarget::Background => (
                                crate::scene_tasks::TaskOwner::Game,
                                TargetKind::Background,
                                "background".into(),
                            ),
                            crate::motion::MotionTarget::Sprite { id } => (
                                crate::scene_tasks::TaskOwner::Game,
                                TargetKind::Sprite,
                                id.clone(),
                            ),
                            crate::motion::MotionTarget::Layer { id, layer } => (
                                crate::scene_tasks::TaskOwner::Game,
                                TargetKind::Layer,
                                format!("{id}/{layer}"),
                            ),
                            crate::motion::MotionTarget::Interface { screen, .. } => {
                                let ui_owner = execution
                                    .resources
                                    .task(ResourceTaskKind::Interface, screen, execution.owner())
                                    .ok_or_else(|| {
                                        self.scene_resource_error(
                                            "Motion interface lacks live authority".into(),
                                        )
                                    })?
                                    .key
                                    .owner
                                    .clone();
                                (ui_owner, TargetKind::Interface, screen.clone())
                            }
                        };
                        let identity = execution
                            .resources
                            .target(&target_owner, target_kind, &alias)
                            .cloned()
                            .ok_or_else(|| {
                                self.scene_resource_error("Motion target lacks incarnation".into())
                            })?;
                        execution
                            .resources
                            .register(
                                crate::scene_resources::resource_key(
                                    owner.clone(),
                                    ResourceTaskKind::Motion,
                                    &target.key(),
                                ),
                                identity,
                                ResourceTaskKind::Motion,
                                None,
                            )
                            .map_err(|message| self.scene_resource_error(message))?;
                    }
                    continue;
                }
                crate::eval::UiCommand::MotionStop { target } => {
                    if let Some(task) =
                        Self::live_resource(next, ResourceTaskKind::Motion, &target.key())
                    {
                        crate::scene_resources::supersede_task(next, &task.key)
                            .map_err(|message| self.scene_resource_error(message))?;
                    }
                    next.motions.tracks.remove(&target.key());
                    continue;
                }
                _ => {}
            }
            let closed_instance=if let crate::eval::UiCommand::Close{name}=&command{self.capture_closing_interface(next,name)?}else{None};
            let mut widget_retirement=if let crate::eval::UiCommand::Close{name}=&command{self.prepare_command_widget_retirement(next,name,closed_instance.as_ref())?}else{None};
            let defer_description=matches!(&command,crate::eval::UiCommand::Open{name,..}if self.consumer_program.native_host_definition(&crate::consumer_schedule::HostDefinitionKind::Screen,name));
            let closing_role = if let crate::eval::UiCommand::Close { name } = &command {
                next.ui
                    .screens
                    .iter()
                    .find(|screen| &screen.name == name)
                    .and_then(|screen| screen.host_role)
            } else {
                None
            };
            let mut lifecycle_owner = owner.clone();
            let mut close_resource = None;
            let mut opened_resource = None;
            if let crate::eval::UiCommand::Open {
                name, host_role, ..
            } = &command
            {
                lifecycle_owner = host_role
                    .map(|role| crate::scene_tasks::TaskOwner::Host {
                        role: role.id().into(),
                    })
                    .unwrap_or(owner.clone());
                if let Some(task) = Self::live_resource(next, ResourceTaskKind::Interface, name) {
                    let old_screen = next
                        .ui
                        .screens
                        .iter()
                        .find(|screen| &screen.name == name)
                        .cloned();
                    crate::scene_resources::remove_target(
                        next,
                        &task.key.owner,
                        TargetKind::Interface,
                        name,
                    )
                    .map_err(|message| self.scene_resource_error(message))?;
                    // The UI library preserves ordering and checks presenter ownership on reopen.
                    if let Some(screen) = old_screen {
                        next.ui.screens.push(screen);
                    }
                }
                opened_resource = Some(name.clone());
            } else if let crate::eval::UiCommand::Close { name } = &command {
                close_resource = Self::live_resource(next, ResourceTaskKind::Interface, name);
                if let Some(task) = &close_resource {
                    lifecycle_owner = task.key.owner.clone();
                }
            }
            let closing_random = if (next.scene_execution.is_some()||next.ui.instance_registry.is_some())
                && matches!(&lifecycle_owner, crate::scene_tasks::TaskOwner::Host { .. }) {
                if let crate::eval::UiCommand::Close { name } = &command {
                    if next.ui.screens.iter().any(|screen|screen.name==*name) {
                        Some(self.capture_visible_ui_random(next, name)?)
                    } else {None}
                } else { None }
            } else { None };
            let mut event=self.evaluate_command_random(next, &owner,
                random_context.as_ref(), |next, random| {
                    let globals = self.global_vars_for_state(next);
                    self.ui_library.with_locale(self.locale.as_ref()).command_deferred(&mut next.ui, command,
                        &self.functions, &globals, random,defer_description)
                        .map_err(|error|self.eval_err(error,"Interface"))
                })?;
            if let Some(task) = close_resource {
                crate::scene_resources::remove_target(
                    next,
                    &task.key.owner,
                    TargetKind::Interface,
                    task.alias(),
                )
                .map_err(|message| self.scene_resource_error(message))?;
            }
            if let Some(name) = opened_resource {
                self.install_owned_resource(
                    next,
                    lifecycle_owner.clone(),
                    ResourceTaskKind::Interface,
                    TargetKind::Interface,
                    &name,
                )?;
                if defer_description{
                    let root=self.prepare_native_screen(next,&name,budget)?;
                    let views=self.describe_interface_structure(next)?;
                    self.ui_library.reconcile_canvas_states_from_views(&mut next.ui,&views).map_err(|error|self.eval_err(error,"Native Screen canvas hydration"))?;
                    if let Some(instance)=next.ui.screens.iter_mut().find(|instance|instance.name==name){instance.focus=views.iter().find(|view|view.name==name).and_then(|view|view.focus.clone());}
                    event=root.map(|root|(crate::ui::UiInput{screen:name.clone(),element:root.id.clone(),kind:rvn_ui::programmable::ScreenEventKind::Open,value:None,key:None},root));
                }
                // Open already hydrates Canvas state. Register its exact tasks
                // before lifecycle callbacks or another validating frame phase.
                if next.scene_execution.is_some() {
                    crate::scene_resources::sync_current(next)
                        .map_err(|message| self.scene_resource_error(message))?;
                }
            }
            if let Some(retirement)=&mut widget_retirement{retirement.random=closing_random.clone().map(|source|std::rc::Rc::new(std::cell::RefCell::new(source)));}
            if let Some((event, component)) = event {
                let mut lifecycle_commands = self.ui_event_owned_budgeted(next,
                    &event, &component, &lifecycle_owner, closing_random, closed_instance, budget)?;
                if let Some(retirement)=&mut widget_retirement{if lifecycle_commands.random.is_some(){retirement.random=lifecycle_commands.random.clone();}}
                if let Some(role) = closing_role {
                    for command in &mut lifecycle_commands.commands {
                        if let crate::eval::UiCommand::Open {
                            host_role,
                            inherit_host: true,
                            ..
                        } = command
                        {
                            *host_role = Some(role);
                        }
                    }
                }
                queue.extend(
                    lifecycle_commands.commands.into_iter().map(|command|
                        (command, lifecycle_commands.owner.clone(), lifecycle_commands.random.clone())),
                );
            }
            if let Some(retirement)=widget_retirement{widget_retirements.push_back(retirement);}
        }
        self.refresh_native_targets(next,&native_targets,budget)?;
        Ok(())
    }

    fn commit_ui_state(&mut self,mut next:GameState)->Result<(),RuntimeError>{
        crate::game_tasks::sync_current(&mut next).map_err(|m|self.scene_resource_error(m))?;
        let cache_before=if self.state.game_audio.is_some()||next.game_audio.is_some(){
            Some(self.interface_view_cache.lock().map_err(|_|self.scene_resource_error("Game audio interface cache unavailable".into()))?.iter().map(|entry|InterfaceViewSnapshot{locale:entry.locale.as_ref().map(LocaleManager::debugger_copy),text_resources:entry.text_resources.clone(),ui:entry.ui.clone(),globals:entry.globals.clone(),views:entry.views.clone()}).collect::<Vec<_>>())
        }else{None};
        let result=self.commit_ui_state_inner(next);
        if result.is_err(){if let Some(cache)=cache_before{*self.interface_view_cache.lock().unwrap_or_else(|poison|poison.into_inner())=cache;}}
        result
    }
    fn commit_ui_state_inner(&mut self, mut next: GameState) -> Result<(), RuntimeError> {
        self.widget_classes.validate_state(&next).map_err(|s|self.scene_resource_error(s))?;
        self.consumer_program.validate_state_binding(&next).map_err(|error|self.eval_err(error,"UI consumer ownership"))?;
        for effect in &next.menu_requests {
            if !next.ui.screens.iter().any(|screen| {
                screen.name == effect.screen
                    && screen.order == effect.screen_order
                    && screen.host_role == effect.host_role
            }) {
                return Err(self.eval_err(
                    EvalError::InvalidMenuRequest(
                        "Menu request screen was closed or replaced during its event".into(),
                    ),
                    "Menu authority",
                ));
            }
            self.renderer
                .validate_menu_request(effect, &next)
                .map_err(|message| {
                    self.eval_err(EvalError::InvalidMenuRequest(message), "Menu authority")
                })?;
        }
        let globals=self.global_vars_for_state(&next);
        self.ui_library.prepare_owner_properties(&mut next.ui,&self.functions,&globals,self.locale.as_ref()).map_err(|e|self.eval_err(e,"UserWidget owner preflight"))?;
        let views = self.describe_interfaces(&next)?;
        crate::ui::published_owner_hosts(&next.ui,&views).map_err(|s|self.scene_resource_error(s))?;
        self.ui_library.with_locale(self.locale.as_ref())
            .reconcile_canvas_states_from_views(&mut next.ui, &views)
            .map_err(|error| self.eval_err(error, "Canvas instance state"))?;
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
        self.normalize_resident_projection_targets(&mut next)?;
        let motions = self.describe_motions(&next)?;
        let layered = self.describe_layered_characters(&next)?;
        if self.narrative.is_some() {return self.commit_scene_state(next);}
        if next.ui.instance_registry.is_some()||next.game_audio.is_some(){
            let video=if next.videos!=self.state.videos{self.next_video_epoch()?}else{self.video_epoch};
            let batch=self.build_scene_batch(next,self.runtime_epoch,self.interface_epoch,video)?;
            return self.commit_built_scene_batch(batch).map_err(|message|self.scene_resource_error(message));
        }
        let epoch = if next.videos != self.state.videos {
            self.next_video_epoch()?
        } else {
            self.video_epoch
        };
        let videos = self.describe_videos(&next, epoch, self.runtime_epoch)?;
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
        for effect in std::mem::take(&mut next.menu_requests) {
            if let Err(message) = self.renderer.menu_request(&effect, &self.state, &next) {
                let _ = self.renderer.update_interfaces(&previous_views);
                let _ = self.renderer.update_layered_characters(&previous_layered);
                let _ = self.renderer.update_motions(&previous_motions);
                let _ = self.renderer.update_videos(&previous_videos);
                let _ = self
                    .renderer
                    .update_accessibility(&self.state.accessibility);
                return Err(self.eval_err(EvalError::InvalidMenuRequest(message), "Menu authority"));
            }
        }
        self.state = next;
        self.video_epoch = epoch;
        Ok(())
    }

    /// Transient Engine identity. Saved epochs alone may restart in a separately
    /// constructed Engine; domain callbacks still require their exact tokens.
    pub fn instance_id(&self) -> u64 { self.engine_instance_id }

    /// Capture at the producing control, before queuing or running any callback.
    pub fn capture_ui_input_source(&self, screen: &str) -> Option<crate::ui_input::UiInputSource> {
        crate::ui_input::capture(&self.state, self.engine_instance_id, self.runtime_epoch, self.interface_epoch, screen)
    }

    pub fn ui_input_source_current(&self, source: &crate::ui_input::UiInputSource) -> bool {
        crate::ui_input::current(&self.state, self.engine_instance_id, self.runtime_epoch, self.interface_epoch, source)
    }

    /// Refuse stale, parked or foreign input before inspecting its event payload.
    /// Accepted events keep the existing atomic handler, Canvas and Host RNG path.
    pub fn interface_event_scoped(&mut self, input: crate::ui_input::ScopedUiInput) -> Result<bool, RuntimeError> {
        if !self.ui_input_source_current(&input.source) || input.event.screen != input.source.screen {
            return Ok(false);
        }
        self.with_host_random_transaction(|engine| engine.interface_event_host_planned(input.event))?;
        Ok(true)
    }

    /// Immediate synchronous compatibility adapter. Queued producers must retain
    /// capture_ui_input_source at production and call interface_event_scoped.
    pub fn interface_event(&mut self, event: crate::ui::UiInput) -> Result<(), RuntimeError> {
        if let Some(source) = self.capture_ui_input_source(&event.screen) {
            self.interface_event_scoped(crate::ui_input::ScopedUiInput { source, event })?;
            Ok(())
        } else if self.state.ui.screens.iter().any(|screen|screen.name == event.screen) {
            // An existing snapshot without valid authority is corruption, not
            // permission to reconstruct a token from its alias.
            Err(self.eval_err(EvalError::InvalidFunction("Interface input instance has no current authority".into()), "Interface event"))
        } else {
            // Preserve the existing diagnostic for an immediate absent address.
            self.with_host_random_transaction(|engine| engine.interface_event_host_planned(event))
        }
    }

    fn interface_event_host_planned(&mut self, event: crate::ui::UiInput) -> Result<(), RuntimeError> {
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
            && crate::ui::top_modal_owner(&self.state.ui).map_err(|message|self.scene_resource_error(message))?.is_some_and(|modal| (view.layer,view.order)<modal)
        {
            return Err(self.eval_err(
                EvalError::InvalidFunction("interface masquée par un écran modal".into()),
                "Interface event",
            ));
        }
        let owner_properties=&self.state.ui.screens.iter().find(|instance|instance.name==event.screen&&instance.order==view.order).ok_or_else(||self.scene_resource_error("Child input lost its actual resident owner".into()))?.owner_properties;
        if !owner_properties.allows_child_event(event.kind){return Err(self.scene_resource_error("Child input is unavailable through its actual owner visibility/enabled policy".into()));}
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
        // Resolve owner and real UI instance before any random evaluation.
        let owner = Self::live_resource(&next,
            crate::scene_resources::ResourceTaskKind::Interface, &event.screen)
            .map(|task| task.key.owner).unwrap_or_else(|| Self::command_owner(&next));
        let commands = self.ui_event_owned_budgeted(&mut next, &event, component,
            &owner, None, None, &mut budget)?;
        self.apply_callback_commands_budgeted(&mut next, commands, &mut budget)?;
        let previous = self.state.clone();
        let historical_previous=self.historical_ui_previous(previous.clone())?;
        self.commit_ui_state(next)?;
        // Focus and unhandled key/click events do not consume rollback steps.
        // A focus handler can still change gameplay: preserve that transition.
        let mut before_ui = previous.ui.story_only();
        let mut after_ui = self.state.ui.story_only();
        for screen in &mut before_ui.screens {
            screen.focus = None;
        }
        for screen in &mut after_ui.screens {
            screen.focus = None;
        }
        if !self.renderer.menu_request_pending()
            && (previous.vars != self.state.vars
                || previous.random != self.state.random
                || before_ui != after_ui)
        {
            self.history.push(historical_previous, None);
        }
        Ok(())
    }

    /// The host calls this only during active game simulation, never from a
    /// wall-clock callback while paused, unfocused or in the save/menu screen.
    /// One frame is atomic; it adds no rollback entry of its own.
    pub fn interface_tick(&mut self, seconds: f64) -> Result<(), RuntimeError> {
        self.with_host_random_transaction(|engine| engine.interface_tick_host_planned(seconds))
    }

    fn interface_tick_host_planned(&mut self, seconds: f64) -> Result<(), RuntimeError> {
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
                    targets.push((view.name.clone(), component.id.clone(),self.capture_ui_input_source(&view.name)));
                }
            });
        }
        if targets.is_empty() {
            return Ok(());
        }
        let mut next = self.state.clone();
        self.ui_library.with_locale(self.locale.as_ref())
            .reconcile_canvas_states_from_views(&mut next.ui, &views)
            .map_err(|error| self.eval_err(error, "Canvas clock"))?;
        let mut budget = crate::ui::CanvasBudget::default();
        for (screen, element,source) in targets {
            if source.as_ref().is_none_or(|source|!crate::ui_input::current(&next,self.engine_instance_id,self.runtime_epoch,self.interface_epoch,source)){continue;}
            // A previous callback may have removed this component or placed a
            // modal over it. Do not deliver a stale tick to a replacement.
            let views = self.describe_interface_structure(&next)?;
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
            let owner = Self::live_resource(&next,
                crate::scene_resources::ResourceTaskKind::Canvas,
                &format!("{}/{}", event.screen, event.element))
                .map(|task| task.key.owner).unwrap_or_else(|| Self::command_owner(&next));
            let commands = self.ui_event_owned_budgeted(&mut next, &event,
                component, &owner, None, None, &mut budget)?;
            self.apply_callback_commands_budgeted(&mut next, commands, &mut budget)?;
        }
        self.commit_ui_state(next)
    }

    /// Evaluate a preview expression in the current narrative scope without
    /// advancing either random stream or mutating the running story.
    pub fn eval_narrative_expression(
        &self,
        expression: &rvn_parser::Expr,
    ) -> Result<Value, RuntimeError> {
        self.functions
            .eval_scoped_with_locale_with_widget_objects(Some(&self.widget_object_context_required(&self.state)?), 
                expression,
                &self.vars_for_eval(),
                &self.lexical_vars_for_eval(),
                &mut self.state.random.clone(), self.locale.as_ref(),
            )
            .map_err(|error| self.eval_err(error, "Narrative preview expression"))
    }

    /// Returns a merged view of regular + persistent variables for evaluation.
    /// Persistent variables are keyed with the `persistent.` prefix.
    fn vars_for_eval(&self) -> HashMap<String, Value> {
        let mut merged = self.lexical_vars_for_eval();
        if let Some(execution) = &self.state.scene_execution {
            merged.extend(execution.pre_scene_temporaries.clone());
            if let Some(active) = &execution.active {
                merged.extend(active.locals.clone());
                merged.extend(active.temporaries.clone());
            }
        }
        self.startup_native_bindings(&mut merged);
        self.narrative_native_bindings(&mut merged);
        merged
    }

    /// Read-only progression is available to lexical presentation/callbacks.
    /// This context never becomes GameState or save data, and contains no caller locals.
    fn global_vars_for_state(&self, state: &GameState) -> HashMap<String, Value> {
        let mut globals = state.vars.clone();
        globals.extend(self.persistent_vars.clone());
        globals
    }

    fn without_persistent_globals(mut globals: HashMap<String, Value>) -> HashMap<String, Value> {
        globals.retain(|name, _| !Self::is_persistent(name));
        globals
    }

    /// Functions/screens/handlers close over game globals, never caller locals.
    fn lexical_vars_for_eval(&self) -> HashMap<String, Value> {
        let mut merged = self.global_vars_for_state(&self.state);
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

    /// Advance only the owners eligible at frame entry. A game timer which
    /// enters a child scene never applies this delta to the new child's tasks.
    /// Parked callers do not tick, including already-due actions.
    pub fn tick_scene_tasks(&mut self, delta_secs: f64) -> Result<bool, RuntimeError> {
        if self.narrative.is_none() {
            return Ok(false);
        }
        if !delta_secs.is_finite() || delta_secs < 0.0 {
            return Err(self.eval_err(
                EvalError::InvalidFunction("Task delta must be finite and nonnegative".into()),
                "Scene clock",
            ));
        }
        let mut next = self.state.clone();
        let execution = next.scene_execution.as_mut().unwrap();
        let owner = execution.owner();
        let has_timer = execution.game_tasks.timer.is_some()
            || execution
                .active
                .as_ref()
                .is_some_and(|active| active.tasks.timer.is_some());
        if !has_timer {
            return Ok(false);
        }
        let game_due = execution
            .game_tasks
            .advance_timer(delta_secs)
            .map_err(|message| self.eval_err(EvalError::InvalidFunction(message), "Game timer"))?;
        let scene_due = match &mut execution.active {
            Some(active) => active.tasks.advance_timer(delta_secs).map_err(|message| {
                self.eval_err(EvalError::InvalidFunction(message), "Scene timer")
            })?,
            None => None,
        };
        let mut action_executed = false;
        if let Some(task) = game_due {
            next.scene_execution.as_mut().unwrap().game_tasks.timer = None;
            next = self.plan_timer_action(next, &task.action)?;
            action_executed = true;
        }
        if let Some(task) = scene_due {
            let execution = next.scene_execution.as_mut().unwrap();
            if execution.owner() == owner {
                execution.active.as_mut().unwrap().tasks.timer = None;
                next = self.plan_timer_action(next, &task.action)?;
                action_executed = true;
            }
        }
        if action_executed {
            self.commit_scene_state(next)?;
        } else {
            let execution = next.scene_execution.as_ref().unwrap();
            execution
                .validate(
                    self.narrative.as_ref().unwrap(),
                    &next.vars,
                    &next.call_stack,
                )
                .map_err(|message| {
                    self.eval_err(EvalError::InvalidFunction(message), "Scene clock")
                })?;
            // Clock-only progress needs no renderer publication.
            self.state = next;
        }
        Ok(true)
    }

    fn plan_timer_action(&self, next: GameState, action: &str) -> Result<GameState, RuntimeError> {
        let parts: Vec<_> = action.split_whitespace().collect();
        match parts.as_slice() {
            ["jump", target] => {
                self.plan_narrative_transfer(next, self.resolve(target)?, None, false)
            }
            ["call", target] => {
                let current = next.pc;
                let displayed_here = next
                    .scene_execution
                    .as_ref()
                    .and_then(|execution| execution.display.as_ref())
                    .is_some_and(|display| display.position.pc == current);
                let continuation = if displayed_here
                    && self.script.get(current).is_some_and(Self::is_interactive)
                {
                    current + 1
                } else {
                    current
                };
                self.plan_narrative_transfer(next, self.resolve(target)?, Some(continuation), false)
            }
            _ => Err(self.eval_err(
                EvalError::InvalidFunction(format!("Invalid timer action: {action:?}")),
                "Timer",
            )),
        }
    }

    pub fn timer_token(&self) -> Option<crate::scene_tasks::TaskToken> {
        if let Some(game)=&self.state.game_tasks {
            let timer=game.tasks.timer.as_ref()?;
            return Some(crate::scene_tasks::TaskToken{runtime_epoch:self.runtime_epoch,key:crate::scene_tasks::TaskKey{owner:crate::scene_tasks::TaskOwner::Game,local_id:"timer".into()},revision:timer.revision});
        }

        use crate::scene_tasks::{TaskKey, TaskOwner, TaskToken};
        let execution = self.state.scene_execution.as_ref()?;
        let (owner, task) = match &execution.active {
            Some(active) if active.tasks.timer.is_some() => (
                TaskOwner::Scene {
                    activation: active.id,
                },
                active.tasks.timer.as_ref()?,
            ),
            _ => (TaskOwner::Game, execution.game_tasks.timer.as_ref()?),
        };
        Some(TaskToken {
            runtime_epoch: self.runtime_epoch,
            key: TaskKey {
                owner,
                local_id: "timer".into(),
            },
            revision: task.revision,
        })
    }

    /// Late or duplicate timer delivery is neutral, before evaluation/RNG/PC.
    pub fn fire_timer(
        &mut self,
        token: &crate::scene_tasks::TaskToken,
    ) -> Result<bool, RuntimeError> {
        use crate::scene_tasks::TaskOwner;
        if self.state.game_tasks.is_some(){return self.fire_standalone_timer(token);}
        if token.runtime_epoch != self.runtime_epoch || token.key.local_id != "timer" {
            return Ok(false);
        }
        let Some(execution) = &self.state.scene_execution else {
            return Ok(false);
        };
        let task = match &token.key.owner {
            TaskOwner::Game => execution.game_tasks.timer.as_ref(),
            TaskOwner::Scene { activation } => execution
                .active
                .as_ref()
                .filter(|active| active.id == *activation)
                .and_then(|active| active.tasks.timer.as_ref()),
            TaskOwner::Host { .. } => None,
        };
        let Some(task) =
            task.filter(|task| task.revision == token.revision && task.elapsed >= task.duration)
        else {
            return Ok(false);
        };
        let action = task.action.clone();
        let mut next = self.state.clone();
        let execution = next.scene_execution.as_mut().unwrap();
        match &token.key.owner {
            TaskOwner::Game => execution.game_tasks.timer = None,
            TaskOwner::Scene { .. } => execution.active.as_mut().unwrap().tasks.timer = None,
            TaskOwner::Host { .. } => unreachable!(),
        }
        let next = self.plan_timer_action(next, &action)?;
        self.commit_scene_state(next)?;
        Ok(true)
    }

    /// Parse a timer action string ("jump label" or "call label") and execute it.
    pub fn execute_timer_action(&mut self, action: &str) -> Result<(), RuntimeError> {
        if let Some(game)=&self.state.game_tasks{let historical=game.historical_timer||game.tasks.timer.is_none()||self.active_timer.is_some();let next=self.plan_standalone_timer_action(self.state.clone(),action,historical)?;return self.commit_audio_state(next);}
        if self.narrative.is_some() {
            let next = self.plan_timer_action(self.state.clone(), action)?;
            return self.commit_scene_state(next);
        }
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
        self.consumer_program.validate_emitted(&self.script).map_err(|error|self.eval_err(error,"Frozen scheduled program"))?;
        self.consumer_program.validate_state_binding(&self.state).map_err(|error|self.eval_err(error,"Scheduled frame ownership"))?;
        self.validate_narrative_loop_gathers(&self.state,self.locale.as_ref())?;
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

#[cfg(test)]
mod interface_view_cache_tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::Ordering;

    #[test]
    fn string_table_revisions_invalidate_cached_views_without_changing_story_or_rng() {
        use crate::string_table::{StringTableAsset, StringTableEntry};
        let asset = |source: &str| StringTableAsset { id: "asset:table".into(), namespace: "VN".into(),
            entries: BTreeMap::from([("key".into(), StringTableEntry {metadata:Default::default(), source: source.into(), developer_notes: String::new() })]) };
        let mut game = game();
        game.functions = FunctionLibrary::from_script(&rvn_parser::parse(r#"screen custom() {
            return component("label","text",{"text":text_to_string(__rvn_string_table_text("asset:table","key"))},[])
        }"#).unwrap()).unwrap();
        game.ui_library = crate::ui::UiLibrary::from_script(&rvn_parser::parse(r#"screen custom() {
            return component("label","text",{"text":text_to_string(__rvn_string_table_text("asset:table","key"))},[])
        }"#).unwrap()).unwrap();
        game.text_resources_mut().publish(asset("First")).unwrap();
        let before = serde_json::to_value(&game.state).unwrap();
        assert_eq!(game.interface_views().unwrap()[0].root.text, "First");
        assert_eq!(game.interface_views().unwrap()[0].root.text, "First");
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 1);
        assert!(!game.text_resources_mut().unload("absent").unwrap());
        game.interface_views().unwrap();
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 1);
        game.text_resources_mut().publish(asset("Second")).unwrap();
        assert_eq!(game.interface_views().unwrap()[0].root.text, "Second");
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 2);
        game.text_resources_mut().unload("asset:table").unwrap();
        assert_eq!(game.interface_views().unwrap()[0].root.text, crate::string_table::MISSING_STRING_TABLE_ENTRY);
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 3);
        assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
    }

    fn game() -> Engine<crate::renderer::TerminalRenderer> {
        let source = r#"
function paint(state,props,frame) {
    return [canvas_rect([frame["time"],state["x"],10,10],[1,0,0,1],radius)]
}
screen custom() {
    return component("root","column",{},[
        component("label","text",{"text":"Hello","text_key":"greeting"},[]),
        component("canvas","canvas",{"draw":"paint","state":{"x":0},"props":{"size":size}},[])])
}
init {set radius=0 set size=20}
label start
"Waiting"
"#;
        let mut game = Engine::new(
            rvn_parser::parse(source).unwrap(),
            crate::renderer::TerminalRenderer,
            16,
        )
        .unwrap();
        game.state.ui.next_order = 1;
        game.state.ui.screens.push(crate::ui::ScreenInstance {
        widget_class:None,
            host_root: false,
            host_role: None,
            name: "custom".into(),
            arguments: Vec::new(),
            modal: false,
            layer: 0,
            order: 0,
            focus: None,
            owner_properties:Default::default(),owner_focus_request:None,
            values: BTreeMap::new(),
            random: crate::random::RandomState::seeded(7),
            consumer_frames:Default::default(),
            consumer_projection:None,
            standard_macro_frame:None,
            canvas_states: BTreeMap::from([(
                "canvas".into(),
                crate::ui::CanvasState {
                    state: Value::Dict(BTreeMap::from([("x".into(), Value::Int(0))])),
                    elapsed: 0.0,
                },
            )]),
        });
        game
    }

    #[test]
    fn pure_view_reads_reuse_exact_inputs_without_mutating_saved_state_or_rng() {
        let game = game();
        let before = serde_json::to_value(&game.state).unwrap();
        let first = game.interface_views().unwrap();
        assert_eq!(first, game.interface_views().unwrap());
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 1);
        let structure = game.describe_interface_structure(&game.state).unwrap();
        assert!(structure[0].root.find("canvas").unwrap().drawing.is_none());
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 1);
        assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
        assert!(game.interface_view_cache.lock().unwrap().len() <= 2);
    }

    #[test]
    fn repeated_current_candidate_recovery_reads_build_only_one_drawing_per_frame() {
        let mut game = game();
        game.interface_views().unwrap();
        for _ in 0..6 {
            let before = game.interface_view_builds.load(Ordering::Relaxed);
            let current = game.interface_views().unwrap();
            let mut next = game.state.clone();
            next.ui.screens[0]
                .canvas_states
                .get_mut("canvas")
                .unwrap()
                .elapsed += 1.0 / 60.0;
            assert_ne!(game.describe_interfaces(&next).unwrap(), current);
            game.describe_interface_structure(&next).unwrap();
            assert_eq!(game.describe_interfaces(&game.state).unwrap(), current);
            assert_eq!(
                game.interface_view_builds.load(Ordering::Relaxed),
                before + 1,
                "Renderer recovery must retain the current snapshot after describing the candidate"
            );
            game.state = next;
            assert_eq!(game.interface_view_cache.lock().unwrap().len(), 2);
        }
    }

    #[test]
    fn public_globals_local_state_and_clock_mutations_invalidate_without_caching_errors() {
        let mut game = game();
        let first = game.interface_views().unwrap();
        game.state.vars.insert("radius".into(), Value::Int(-1));
        assert!(game.interface_views().is_err());
        assert!(game.interface_views().is_err());
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 3);
        game.state.vars.insert("radius".into(), Value::Int(0));
        assert_eq!(game.interface_views().unwrap(), first);
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 3);
        game.state.ui.screens[0]
            .canvas_states
            .get_mut("canvas")
            .unwrap()
            .elapsed = 0.1;
        let timed = game.interface_views().unwrap();
        assert_ne!(timed, first);
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 4);
        game.state.ui.screens[0]
            .canvas_states
            .get_mut("canvas")
            .unwrap()
            .state = Value::Dict(BTreeMap::from([("x".into(), Value::Int(5))]));
        assert_ne!(game.interface_views().unwrap(), timed);
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 5);
        assert_eq!(game.interface_view_cache.lock().unwrap().len(), 2);
    }

    #[test]
    fn locale_changes_rebuild_contextual_views_without_changing_rng() {
        let mut game = game();
        let before = serde_json::to_value(&game.state).unwrap();
        assert_eq!(
            game.interface_views().unwrap()[0]
                .root
                .find("label")
                .unwrap()
                .text,
            "Hello"
        );
        game.locale = Some(LocaleManager::from_tables(
            "",
            "en",
            "fr",
            vec!["en".into(), "fr".into()],
            HashMap::from([
                (
                    "en".into(),
                    crate::locale::LocaleTable {
                        lang: "en".into(),
                        strings: HashMap::from([("greeting".into(), "Hello".into())]),
                    },
                ),
                (
                    "fr".into(),
                    crate::locale::LocaleTable {
                        lang: "fr".into(),
                        strings: HashMap::from([("greeting".into(), "Bonjour".into())]),
                    },
                ),
            ]),
        ));
        assert_eq!(
            game.interface_views().unwrap()[0]
                .root
                .find("label")
                .unwrap()
                .text,
            "Bonjour"
        );
        game.locale.as_mut().unwrap().set_language("en").unwrap();
        assert_eq!(
            game.interface_views().unwrap()[0]
                .root
                .find("label")
                .unwrap()
                .text,
            "Hello"
        );
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 3);
        assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
    }

    #[test]
    fn cache_input_equality_is_bit_exact_for_nested_floats_and_canvas_clocks() {
        let mut game = game();
        game.state.vars.insert(
            "nested".into(),
            Value::List(vec![Value::Dict(BTreeMap::from([(
                "number".into(),
                Value::Float(0.0),
            )]))]),
        );
        game.interface_views().unwrap();
        game.state.vars.insert(
            "nested".into(),
            Value::List(vec![Value::Dict(BTreeMap::from([(
                "number".into(),
                Value::Float(-0.0),
            )]))]),
        );
        game.interface_views().unwrap();
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 2);
        game.state.ui.screens[0]
            .canvas_states
            .get_mut("canvas")
            .unwrap()
            .elapsed = -0.0;
        game.interface_views().unwrap();
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 3);
        game.state.ui.screens[0]
            .canvas_states
            .get_mut("canvas")
            .unwrap()
            .state = Value::Dict(BTreeMap::from([("x".into(), Value::Float(0.0))]));
        game.interface_views().unwrap();
        game.state.ui.screens[0]
            .canvas_states
            .get_mut("canvas")
            .unwrap()
            .state = Value::Dict(BTreeMap::from([("x".into(), Value::Float(-0.0))]));
        game.interface_views().unwrap();
        assert_eq!(game.interface_view_builds.load(Ordering::Relaxed), 5);
    }
}

#[cfg(test)]
#[path="engine_empty_root_tests_0942.rs"]
mod empty_root_tests_0942;

#[cfg(test)]
#[path="engine_function_library_runtime_tests1023.rs"]
mod function_library_runtime1023;
