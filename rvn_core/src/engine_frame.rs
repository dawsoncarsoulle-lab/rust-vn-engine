//! One coherent narrative frame with exact transient callback capabilities.
//! Core owns effect clocks; Host canvas time has a separate route.
//! Public wrappers below never call an Engine mutating feedback/tick method.

use super::*;
use crate::scene::ActivationId;
use crate::scene_resources::{ResourceTaskKind, ResourceToken};
use crate::scene_tasks::{TaskKey, TaskOwner, TaskToken, TimerTask};
use crate::ui::CanvasBudget;
use crate::video::{Feedback, Playback};
use rvn_ui::programmable::{ComponentKind, ScreenEventKind};

/// These are transient decoder/control deliveries, never save data.
#[derive(Clone, Debug)]
pub enum GameVideoFeedback {
    Report(Feedback),
    Resume,
}

#[derive(Clone, Debug)]
pub struct GameVideoReport {
    pub token: ResourceToken,
    pub feedback: GameVideoFeedback,
}

/// The caller retains producer order. Positions are applied first, and the
/// non-position deliveries retain their original relative order in phase two.
#[derive(Clone, Debug, Default)]
pub struct GameFrameReports {
    pub videos: Vec<GameVideoReport>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameFrameOutcome {
    pub progressed: bool,
    pub published: bool,
    pub scene_changed: bool,
    pub waiting_released: bool,
    pub accepted_reports: usize,
    pub ignored_reports: usize,
    pub callbacks: usize,
}

/// Host operations have their own instant; Story clocks never receive its delta.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostFrameOutcome {
    pub progressed: bool,
    pub published: bool,
    pub callbacks: usize,
    pub completed_effects: usize,
}


/// Pure current/parked/stale authority, independent of decoder payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VideoReportAuthority { Current(TaskOwner), Parked, Stale }
/// Consumed means the admitted route processed this input ordinal. It may be
/// neutral after a previous callback revoked it; it does not claim a callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoReportDisposition { Consumed, RejectedStale, Deferred }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoReportReceipt { pub ordinal: usize, pub disposition: VideoReportDisposition }
#[derive(Clone, Debug, Default)]
pub struct HostFrameReports { pub videos: Vec<GameVideoReport> }
#[derive(Clone, Debug)]
pub struct HostFrameReceiptOutcome {
    pub frame: HostFrameOutcome,
    pub receipts: Vec<VideoReportReceipt>,
    pub accepted_reports: usize,
    pub completed_motions: usize,
}
#[derive(Clone, Debug)]
pub struct GameFrameReceiptOutcome { pub frame: GameFrameOutcome, pub receipts: Vec<VideoReportReceipt> }

#[derive(Clone, Copy)]
enum CanvasClockScope {
    Story,
    Host,
}

struct FrameCanvas {
    screen: String,
    element: String,
    order: u64,
    owner: TaskOwner,
    // None is permitted only for a real Host instance in a legacy engine.
    // An explicit-scene engine must use the registry's actual identity.
    token: Option<ResourceToken>,
}

struct FrameTimer {
    token: TaskToken,
    task: TimerTask,
}

struct FramePlan {
    next: GameState,
    canvases: Vec<FrameCanvas>,
    motions_due: Vec<ResourceToken>,
    effects_due: Vec<ResourceToken>,
    game_timer: Option<FrameTimer>,
    scene_timer: Option<FrameTimer>,
    canvas_seconds: f64,
    outcome: GameFrameOutcome,
}

impl<R: Renderer> Engine<R> {
    /// Pure callback authority classification. A real parked descriptor is
    /// deferred; an already-replaced pre-Park revision is stale. No payload
    /// is inspected by this function and no identity is reconstructed by alias.
    pub fn video_report_authority(
        &self,
        token: &ResourceToken,
    ) -> Result<VideoReportAuthority, RuntimeError> {
        if token.task.runtime_epoch != self.runtime_epoch { return Ok(VideoReportAuthority::Stale); }
        let Some(execution) = &self.state.scene_execution else { return Ok(VideoReportAuthority::Stale); };
        let Some(task) = execution.resources.tasks.iter().find(|task|
            task.key == token.task.key && task.revision == token.task.revision
                && task.target == token.target) else { return Ok(VideoReportAuthority::Stale); };
        if task.kind != ResourceTaskKind::Video || token.target.kind != crate::scene_resources::TargetKind::Video {
            return Err(self.frame_error("Exact video report belongs to another resource family"));
        }
        if !execution.resources.targets.iter().any(|record| record.identity == task.target)
            || task.dependencies.iter().any(|dependency|
                !execution.resources.targets.iter().any(|record| record.identity == *dependency)) {
            return Ok(VideoReportAuthority::Stale);
        }
        if task.parked { return Ok(VideoReportAuthority::Parked); }
        if !self.frame_exact_resource(&self.state, token, ResourceTaskKind::Video) {
            return Err(self.frame_error("An exact live video descriptor has invalid owner authority"));
        }
        Ok(VideoReportAuthority::Current(task.key.owner.clone()))
    }

    /// Additive queue receipts. They address the caller's input ordinals, not
    /// just a token, so repeated Position/End rows remain distinguishable.
    pub fn tick_game_frame_with_receipts(
        &mut self,
        seconds: f64,
        reports: GameFrameReports,
    ) -> Result<GameFrameReceiptOutcome, RuntimeError> {
        if reports.videos.len() > crate::scene_resources::MAX_RESOURCES {
            return Err(self.frame_error("Too many video reports in one Game frame"));
        }
        let mut submitted = Vec::new();
        let mut receipts = Vec::with_capacity(reports.videos.len());
        for (ordinal, report) in reports.videos.into_iter().enumerate() {
            let disposition = match self.video_report_authority(&report.token)? {
                VideoReportAuthority::Stale => VideoReportDisposition::RejectedStale,
                VideoReportAuthority::Current(TaskOwner::Game | TaskOwner::Scene { .. }) => {
                    submitted.push(report);
                    VideoReportDisposition::Consumed
                }
                _ => VideoReportDisposition::Deferred,
            };
            receipts.push(VideoReportReceipt { ordinal, disposition });
        }
        let frame = self.tick_game_frame(seconds, GameFrameReports { videos: submitted })?;
        Ok(GameFrameReceiptOutcome { frame, receipts })
    }

    /// An admitted Host instant prepares Motion/Effect/Canvas clocks and all
    /// Video positions before callbacks, then accepts one checked publication.
    /// Queue ownership/gates remain with the caller; blocked routes are never
    /// called merely to execute an Error handler at a zero delta.
    pub fn tick_host_frame_with_reports(
        &mut self,
        seconds: f64,
        reports: HostFrameReports,
    ) -> Result<HostFrameReceiptOutcome, RuntimeError> {
        self.with_host_random_transaction(|engine|
            engine.plan_host_media_frame(seconds, reports, &[]))
    }

    fn plan_host_media_frame(
        &mut self,
        seconds: f64,
        reports: HostFrameReports,
        audio_reports: &[crate::audio_playback::AudioReport],
    ) -> Result<HostFrameReceiptOutcome, RuntimeError> {
        if !seconds.is_finite() || !(0.0..=0.25).contains(&seconds) {
            return Err(self.frame_error("Host delta must be finite and between 0 and 0.25 seconds"));
        }
        if reports.videos.len() > crate::scene_resources::MAX_RESOURCES {
            return Err(self.frame_error("Too many video reports in one Host frame"));
        }
        if self.narrative.is_some() && self.state.scene_execution.is_none() {
            return Err(self.frame_error("Narrative engine has no scene execution"));
        }
        if self.state.scene_execution.is_some() {
            crate::scene_resources::validate_current_resource_state(&self.state)
                .map_err(|message| self.scene_resource_error(message))?;
        }
        let mut deliveries = Vec::new();
        let mut receipts = Vec::with_capacity(reports.videos.len());
        for (ordinal, report) in reports.videos.into_iter().enumerate() {
            let disposition = match self.video_report_authority(&report.token)? {
                VideoReportAuthority::Stale => VideoReportDisposition::RejectedStale,
                VideoReportAuthority::Current(TaskOwner::Host { .. }) => {
                    deliveries.push(report);
                    VideoReportDisposition::Consumed
                }
                _ => VideoReportDisposition::Deferred,
            };
            receipts.push(VideoReportReceipt { ordinal, disposition });
        }
        let mut captured = self.state.scene_execution.as_ref().map(|execution|
            execution.resources.tasks.iter().filter(|task|
                !task.parked && matches!(&task.key.owner, TaskOwner::Host { .. })
                    && matches!(task.kind, ResourceTaskKind::Motion | ResourceTaskKind::Effect))
                .filter_map(|task| execution.resources.effect_token(self.runtime_epoch,
                    &task.key, execution.owner()).map(|token| (token, task.kind, task.alias().to_owned())))
                .collect::<Vec<_>>()).unwrap_or_default();
        captured.sort_by_key(|(token, kind, _)| {
            let role = match &token.task.key.owner { TaskOwner::Host { role } => role.clone(), _ => String::new() };
            let family = if *kind == ResourceTaskKind::Motion { 0 } else { 1 };
            (role, family, token.task.key.local_id.clone())
        });
        let mut next = self.state.clone();
        let mut frame = HostFrameOutcome::default();
        let mut accepted_reports = 0;
        // Every admitted current position is checked before any handler. A
        // paused/blocked track retains the existing Core feedback policy.
        for report in &deliveries {
            let GameVideoFeedback::Report(Feedback::Position { .. }) = &report.feedback else { continue; };
            if !self.frame_exact_resource(&next, &report.token, ResourceTaskKind::Video) { continue; }
            let id = &report.token.target.alias;
            let track = next.videos.tracks.get(id).ok_or_else(||
                self.frame_error("Exact Host video task has no live descriptor"))?;
            if track.finished() || track.playback != Playback::Playing { continue; }
            if !self.frame_video_target_current(&next, id, track)? { continue; }
            next.videos.feedback(id, match &report.feedback {
                GameVideoFeedback::Report(feedback) => feedback.clone(),
                GameVideoFeedback::Resume => unreachable!(),
            }).map_err(|message| self.frame_error(&message))?;
            accepted_reports += 1;
            frame.progressed = true;
        }
        frame.progressed |= crate::audio_playback::advance_clocks(&mut next, self.runtime_epoch, crate::audio_playback::AudioClockRoute::Host, seconds).map_err(|message|self.scene_resource_error(message))?;
        let canvases = self.prepare_frame_canvases(&mut next, CanvasClockScope::Host, seconds)?;
        frame.progressed |= !canvases.is_empty();
        let mut motions_due = Vec::new();
        let mut effects_due = Vec::new();
        for (token, kind, alias) in captured {
            if !self.frame_exact_resource(&next, &token, kind) { continue; }
            if kind == ResourceTaskKind::Motion {
                let track = next.motions.tracks.get_mut(&alias).ok_or_else(||
                        self.frame_error("Exact Host motion task has no live descriptor"))?;
                let was_running = track.running;
                if track.running {
                    let elapsed = track.elapsed + seconds;
                    if !elapsed.is_finite() { return Err(self.frame_error("Host motion clock overflow")); }
                    track.elapsed = elapsed;
                    let end = track.definition.validate().map_err(|message| self.frame_error(&message))?.seconds;
                    if let Some(end) = end {
                        if track.elapsed >= end { track.elapsed = end; track.running = false; }
                    } else if track.elapsed > 1_000_000.0 {
                        if let rvn_ui::motion::Motion::Repeat { motion, .. } = &track.definition {
                            let cycle = motion.validate().map_err(|message| self.frame_error(&message))?
                                .seconds.ok_or_else(|| self.frame_error("Endless Host motion has no cycle"))?;
                            track.elapsed %= cycle;
                        }
                    }
                    track.validate().map_err(|message| self.frame_error(&message))?;
                    frame.progressed |= seconds > 0.0 || !track.running;
                }
                let terminal = !track.running;
                if terminal && (was_running || next.scene_execution.as_ref().is_some_and(|execution|
                    execution.resources.waits.iter().any(|wait| wait.key == token.task.key
                        && wait.revision == token.task.revision && wait.target == token.target))) {
                    motions_due.push(token);
                }
            } else {
                let effect = next.scene_execution.as_mut().unwrap().resources.tasks.iter_mut()
                    .find(|task| task.key == token.task.key && task.revision == token.task.revision
                        && task.target == token.target).and_then(|task| task.effect.as_mut())
                    .ok_or_else(|| self.frame_error("Exact Host effect has no descriptor"))?;
                let clock = effect.clock.advanced(seconds).map_err(|message| self.frame_error(&message))?;
                frame.progressed |= clock != effect.clock;
                effect.clock = clock;
                effect.validate().map_err(|message| self.frame_error(&message))?;
                if effect.clock.is_due() { effects_due.push(token); }
            }
        }
        let mut budget = CanvasBudget::default();
        let mut completed_motions = 0;
        for token in motions_due {
            if !self.frame_exact_resource(&next, &token, ResourceTaskKind::Motion) { continue; }
            crate::scene_resources::release_waiters(&mut next, &token, self.runtime_epoch)
                .map_err(|message| self.scene_resource_error(message))?;
            completed_motions += 1;
        }
        for canvas in &canvases {
            if self.dispatch_frame_canvas(&mut next, canvas, seconds, &mut budget)? { frame.callbacks += 1; }
        }
        // Producer order among non-position deliveries is retained. Every
        // capability/physical UI dependency is rechecked after earlier handlers.
        for report in deliveries {
            if matches!(&report.feedback, GameVideoFeedback::Report(Feedback::Position { .. })) { continue; }
            if !self.frame_exact_resource(&next, &report.token, ResourceTaskKind::Video) { continue; }
            let id = &report.token.target.alias;
            let Some(track) = next.videos.tracks.get(id) else { continue; };
            if track.finished() || !self.frame_video_target_current(&next, id, track)? { continue; }
            match &report.feedback {
                GameVideoFeedback::Resume => {
                    self.apply_ui_commands_budgeted_owned(&mut next,
                        vec![crate::eval::UiCommand::VideoResume { name: id.clone() }],
                        &mut budget, report.token.task.key.owner.clone())?;
                }
                GameVideoFeedback::Report(feedback) => {
                    if matches!(feedback, Feedback::End) && track.playback != Playback::Playing { continue; }
                    let handler = next.videos.feedback(id, feedback.clone())
                        .map_err(|message| self.frame_error(&message))?;
                    if next.videos.tracks[id].finished() {
                        crate::scene_resources::release_waiters(&mut next, &report.token, self.runtime_epoch)
                            .map_err(|message| self.scene_resource_error(message))?;
                    }
                    if let Some(handler) = handler {
                        let commands = self.video_handler_budgeted(&mut next, id, &handler, &mut budget)?;
                        self.apply_callback_commands_budgeted(&mut next, commands, &mut budget)?;
                        frame.callbacks += 1;
                    }
                }
            }
            accepted_reports += 1;
            frame.progressed = true;
        }
        frame.progressed |= self.apply_audio_deliveries(&mut next, audio_reports)?;
        frame.progressed |= crate::audio_playback::complete_due_clocks(&mut next,self.runtime_epoch,crate::audio_playback::AudioClockRoute::Host).map_err(|message|self.scene_resource_error(message))?;
        for token in effects_due {
            if !self.frame_host_effect_current(&next, &token) { continue; }
            if crate::scene_resources::complete_due_effect(&mut next, &token, self.runtime_epoch)
                .map_err(|message| self.scene_resource_error(message))?.is_some() {
                frame.completed_effects += 1;
                frame.progressed = true;
            }
        }
        if frame.progressed {
            self.commit_ui_state(next)?;
            frame.published = true;
        }
        Ok(HostFrameReceiptOutcome { frame, receipts, accepted_reports, completed_motions })
    }

    /// Advance a complete narrative simulation frame, then dispatch its exact
    /// entry-time capabilities, and publish at most one coherent scene batch.
    /// Effect bases must have been neutrally captured before this call; Core
    /// owns SavedEffect.clock.elapsed and no ECS clock is accepted here.
    pub fn tick_game_frame(
        &mut self,
        seconds: f64,
        reports: GameFrameReports,
    ) -> Result<GameFrameOutcome, RuntimeError> {
        self.with_host_random_transaction(|engine| engine.tick_game_frame_host_planned(seconds, reports, &[]))
    }

    fn tick_game_frame_host_planned(
        &mut self,
        seconds: f64,
        reports: GameFrameReports,
        audio_reports: &[crate::audio_playback::AudioReport],
    ) -> Result<GameFrameOutcome, RuntimeError> {
        if self.narrative.is_none() {
            if self.state.game_audio.is_some()||self.state.game_tasks.is_some(){return self.tick_standalone_game_frame(seconds,reports,audio_reports);}
            return Err(self.frame_error("Game frames require an explicit scene program"));
        }
        if !seconds.is_finite() || !(0.0..=3600.0).contains(&seconds) {
            return Err(
                self.frame_error("Game delta must be finite and between 0 and 3600 seconds")
            );
        }
        if reports.videos.len() > crate::scene_resources::MAX_RESOURCES {
            return Err(self.frame_error("Too many video reports in one Game frame"));
        }
        let initial_owner = self
            .state
            .scene_execution
            .as_ref()
            .ok_or_else(|| self.frame_error("Narrative engine has no scene execution"))?
            .owner();
        let initial_pc = self.state.pc;
        let initial_stack = self.state.call_stack.clone();
        let was_waiting =
            self.state.motions.waiting.is_some() || self.state.videos.waiting.is_some();
        let mut plan = self.plan_game_frame_clocks(seconds, &reports)?;
        plan.outcome.progressed |= crate::audio_playback::advance_clocks(&mut plan.next, self.runtime_epoch, crate::audio_playback::AudioClockRoute::Story, seconds).map_err(|message|self.scene_resource_error(message))?;
        let mut budget = CanvasBudget::default();

        // Preserve the runner's existing semantic order, after preparing all
        // families: Motion completion, Canvas, Video events, Game/Scene timer.
        for token in &plan.motions_due {
            if self.frame_resource_current(&plan.next, token, ResourceTaskKind::Motion) {
                crate::scene_resources::release_waiters(&mut plan.next, token, self.runtime_epoch)
                    .map_err(|message| self.scene_resource_error(message))?;
                plan.outcome.progressed = true;
            }
        }
        for canvas in &plan.canvases {
            if self.dispatch_frame_canvas(
                &mut plan.next,
                canvas,
                plan.canvas_seconds,
                &mut budget,
            )? {
                plan.outcome.callbacks += 1;
            }
        }
        for report in reports.videos {
            if matches!(
                &report.feedback,
                GameVideoFeedback::Report(Feedback::Position { .. })
            ) {
                continue;
            }
            if self.plan_frame_video_delivery(
                &mut plan.next,
                &report,
                &mut budget,
                &mut plan.outcome,
            )? {
                plan.outcome.accepted_reports += 1;
                plan.outcome.progressed = true;
            } else {
                plan.outcome.ignored_reports += 1;
            }
        }
        plan.outcome.progressed |= self.apply_audio_deliveries(&mut plan.next, audio_reports)?;
        for timer in [plan.game_timer.as_ref(), plan.scene_timer.as_ref()]
            .into_iter()
            .flatten()
        {
            if self.frame_timer_current(&plan.next, timer) {
                let execution = plan.next.scene_execution.as_mut().unwrap();
                match &timer.token.key.owner {
                    TaskOwner::Game => execution.game_tasks.timer = None,
                    TaskOwner::Scene { .. } => {
                        execution.active.as_mut().unwrap().tasks.timer = None
                    }
                    TaskOwner::Host { .. } => unreachable!("Host timer excluded from Game frame"),
                }
                plan.next = self.plan_timer_action(plan.next, &timer.task.action)?;
                plan.outcome.callbacks += 1;
                plan.outcome.progressed = true;
            }
        }
        plan.outcome.progressed |= crate::audio_playback::complete_due_clocks(&mut plan.next,self.runtime_epoch,crate::audio_playback::AudioClockRoute::Story).map_err(|message|self.scene_resource_error(message))?;
        // A preceding callback may have removed/replaced/parked a due effect.
        // The transaction checks the same token and removes only its terminal
        // operation. A parked Scene keeps its terminal elapsed until active;
        // finishing an effect never removes a current Game sprite.
        for token in &plan.effects_due {
            if self.frame_resource_current(&plan.next, token, ResourceTaskKind::Effect) {
                let completed = crate::scene_resources::complete_due_effect(
                    &mut plan.next,
                    token,
                    self.runtime_epoch,
                )
                .map_err(|message| self.scene_resource_error(message))?;
                plan.outcome.progressed |= completed.is_some();
            }
        }
        let owner = plan.next.scene_execution.as_ref().unwrap().owner();
        plan.outcome.scene_changed = initial_owner != owner
            || initial_pc != plan.next.pc
            || initial_stack != plan.next.call_stack;
        plan.outcome.waiting_released = was_waiting
            && plan.next.motions.waiting.is_none()
            && plan.next.videos.waiting.is_none();
        if plan.outcome.progressed {
            // This is the only authoritative installation/publication. Its
            // validation must precede every state/epoch/output assignment.
            // Normalize trees changed through globals as well as UiCommands:
            // hydrate new Canvas instances, repair focus and cancel UI-bound
            // Motion/Video targets before the one narrative publication.
            self.commit_ui_state(plan.next)?;
            plan.outcome.published = true;
        }
        Ok(plan.outcome)
    }

    /// Additive audio terminal admission on the same candidate and publication
    /// as the existing frame. Blocked routes keep current terminals deferred.
    pub fn tick_game_frame_with_audio(&mut self, seconds:f64, reports:GameFrameReports, audio:Vec<crate::audio_playback::AudioReport>, enabled:bool)
        ->Result<(GameFrameOutcome,Vec<super::AudioReportReceipt>),RuntimeError> {
        let (deliveries,receipts)=self.prepare_audio_reports(audio,crate::audio_playback::AudioClockRoute::Story,enabled)?;
        if !enabled {return Ok((GameFrameOutcome::default(),receipts));}
        self.with_host_random_transaction(|engine| {
            let frame=engine.tick_game_frame_host_planned(seconds,reports,&deliveries)?;
            Ok((frame,receipts))
        })
    }
    /// Preserve the existing video ordinal receipts while both report families
    /// run on one candidate, with one final renderer publication.
    pub fn tick_game_frame_with_receipts_and_audio(&mut self, seconds:f64, reports:GameFrameReports, audio:Vec<crate::audio_playback::AudioReport>, enabled:bool)
        ->Result<(GameFrameReceiptOutcome,Vec<super::AudioReportReceipt>),RuntimeError> {
        if reports.videos.len()>crate::scene_resources::MAX_RESOURCES {return Err(self.frame_error("Too many video reports in one Game frame"));}
        let mut submitted=Vec::new();let mut receipts=Vec::with_capacity(reports.videos.len());
        for (ordinal,report) in reports.videos.into_iter().enumerate() {
            let disposition=match self.video_report_authority(&report.token)? {
                VideoReportAuthority::Stale=>VideoReportDisposition::RejectedStale,
                VideoReportAuthority::Current(TaskOwner::Game|TaskOwner::Scene{..}) if enabled=>{submitted.push(report);VideoReportDisposition::Consumed},
                _=>VideoReportDisposition::Deferred,
            };
            receipts.push(VideoReportReceipt{ordinal,disposition});
        }
        let (frame,audio_receipts)=self.tick_game_frame_with_audio(seconds,GameFrameReports{videos:submitted},audio,enabled)?;
        Ok((GameFrameReceiptOutcome{frame,receipts},audio_receipts))
    }
    pub fn tick_host_frame_with_audio(&mut self, seconds:f64, reports:HostFrameReports, audio:Vec<crate::audio_playback::AudioReport>, enabled:bool)
        ->Result<(HostFrameReceiptOutcome,Vec<super::AudioReportReceipt>),RuntimeError> {
        let (deliveries,receipts)=self.prepare_audio_reports(audio,crate::audio_playback::AudioClockRoute::Host,enabled)?;
        if !enabled {
            if reports.videos.len()>crate::scene_resources::MAX_RESOURCES {return Err(self.frame_error("Too many video reports in one Host frame"));}
            let video_receipts=reports.videos.iter().enumerate().map(|(ordinal,report)| {
                let disposition=match self.video_report_authority(&report.token)? {VideoReportAuthority::Stale=>VideoReportDisposition::RejectedStale,_=>VideoReportDisposition::Deferred};
                Ok(VideoReportReceipt{ordinal,disposition})
            }).collect::<Result<Vec<_>,RuntimeError>>()?;
            return Ok((HostFrameReceiptOutcome{frame:HostFrameOutcome::default(),receipts:video_receipts,accepted_reports:0,completed_motions:0},receipts));
        }
        self.with_host_random_transaction(|engine| {
            let frame=engine.plan_host_media_frame(seconds,reports,&deliveries)?;
            Ok((frame,receipts))
        })
    }

    /// One Host instant: prepare exact effect/Canvas clocks, then callbacks,
    /// terminal operations and one checked UI publication. Story clocks do
    /// not receive this delta. Projection never advances these clocks.
    pub fn tick_host_frame(&mut self, seconds: f64) -> Result<HostFrameOutcome, RuntimeError> {
        self.tick_host_frame_with_reports(seconds, HostFrameReports::default()).map(|outcome| outcome.frame)
    }

    /// Existing callers retain one Host call and their public return shape.
    /// A zero delta can retire an already-terminal exact Host operation.
    pub fn interface_tick_host(&mut self, seconds: f64) -> Result<(), RuntimeError> {
        self.tick_host_frame(seconds).map(|_| ())
    }

    fn frame_host_effect_current(&self, state: &GameState, token: &ResourceToken) -> bool {
        matches!(&token.task.key.owner, TaskOwner::Host { .. })
            && self.frame_exact_resource(state, token, ResourceTaskKind::Effect)
    }

    fn frame_error(&self, message: &str) -> RuntimeError {
        self.eval_err(EvalError::InvalidFunction(message.into()), "Game frame")
    }

    fn frame_story_owner(owner: &TaskOwner, active: Option<ActivationId>) -> bool {
        match owner {
            TaskOwner::Game => true,
            TaskOwner::Scene { activation } => Some(*activation) == active,
            TaskOwner::Host { .. } => false,
        }
    }

    fn frame_resource_current(
        &self,
        state: &GameState,
        token: &ResourceToken,
        kind: ResourceTaskKind,
    ) -> bool {
        let Some(execution) = &state.scene_execution else {
            return false;
        };
        Self::frame_story_owner(&token.task.key.owner, execution.owner())
            && self.frame_exact_resource(state, token, kind)
    }

    fn frame_exact_resource(
        &self,
        state: &GameState,
        token: &ResourceToken,
        kind: ResourceTaskKind,
    ) -> bool {
        let Some(execution) = &state.scene_execution else {
            return false;
        };
        execution
            .resources
            .accepts(token, self.runtime_epoch, execution.owner())
            && execution.resources.tasks.iter().any(|task| {
                task.key == token.task.key
                    && task.kind == kind
                    && task.revision == token.task.revision
                    && task.target == token.target
                    && task.dependencies.iter().all(|dependency| {
                        execution
                            .resources
                            .targets
                            .iter()
                            .any(|record| record.identity == *dependency)
                    })
            })
    }

    fn plan_game_frame_clocks(
        &self,
        seconds: f64,
        reports: &GameFrameReports,
    ) -> Result<FramePlan, RuntimeError> {
        crate::scene_resources::validate_current_resource_state(&self.state)
            .map_err(|message| self.scene_resource_error(message))?;
        let mut plan = FramePlan {
            next: self.state.clone(),
            canvases: Vec::new(),
            motions_due: Vec::new(),
            effects_due: Vec::new(),
            game_timer: None,
            scene_timer: None,
            canvas_seconds: seconds.min(0.25),
            outcome: GameFrameOutcome::default(),
        };
        let execution = plan.next.scene_execution.as_ref().unwrap();
        let active = execution.owner();
        let mut resources: Vec<_> = execution
            .resources
            .tasks
            .iter()
            .filter(|task| !task.parked && Self::frame_story_owner(&task.key.owner, active))
            .filter(|task| {
                matches!(
                    task.kind,
                    ResourceTaskKind::Motion | ResourceTaskKind::Effect
                )
            })
            .filter_map(|task| {
                execution
                    .resources
                    .effect_token(self.runtime_epoch, &task.key, active)
                    .map(|token| (task.kind, task.alias().to_owned(), token))
            })
            .collect();
        resources.sort_by(|(_, _, left), (_, _, right)| {
            let rank = |owner: &TaskOwner| match owner {
                TaskOwner::Game => (0, 0),
                TaskOwner::Scene { activation } => (1, activation.0),
                TaskOwner::Host { .. } => (2, 0),
            };
            (rank(&left.task.key.owner), &left.task.key.local_id)
                .cmp(&(rank(&right.task.key.owner), &right.task.key.local_id))
        });

        // Ignore stale positions before touching their untrusted payload. All
        // accepted positions are present before any handler can inspect state.
        for report in &reports.videos {
            let GameVideoFeedback::Report(Feedback::Position { seconds, duration }) =
                &report.feedback
            else {
                continue;
            };
            if !self.frame_resource_current(&plan.next, &report.token, ResourceTaskKind::Video)
                || plan
                    .next
                    .videos
                    .tracks
                    .get(&report.token.target.alias)
                    .is_none_or(|track| track.finished() || track.playback != Playback::Playing)
            {
                plan.outcome.ignored_reports += 1;
                continue;
            }
            if !self.frame_video_target_current(
                &plan.next,
                &report.token.target.alias,
                &plan.next.videos.tracks[&report.token.target.alias],
            )? {
                plan.outcome.ignored_reports += 1;
                continue;
            }
            plan.next
                .videos
                .feedback(
                    &report.token.target.alias,
                    Feedback::Position {
                        seconds: *seconds,
                        duration: *duration,
                    },
                )
                .map_err(|message| self.frame_error(&message))?;
            plan.outcome.accepted_reports += 1;
            plan.outcome.progressed = true;
        }

        plan.canvases = self.prepare_frame_canvases(
            &mut plan.next,
            CanvasClockScope::Story,
            plan.canvas_seconds,
        )?;
        plan.outcome.progressed |= !plan.canvases.is_empty();
        for (kind, alias, token) in resources {
            if !self.frame_resource_current(&plan.next, &token, kind) {
                continue;
            }
            if kind == ResourceTaskKind::Motion {
                let Some(track) = plan.next.motions.tracks.get_mut(&alias) else {
                    return Err(self.frame_error("Exact motion task has no live descriptor"));
                };
                if track.running {
                    track.elapsed += seconds;
                    if !track.elapsed.is_finite() {
                        return Err(self.frame_error("Motion clock overflow"));
                    }
                    let end = track
                        .definition
                        .validate()
                        .map_err(|message| self.frame_error(&message))?
                        .seconds;
                    if let Some(end) = end {
                        if track.elapsed >= end {
                            track.elapsed = end;
                            track.running = false;
                        }
                    } else if track.elapsed > 1_000_000.0 {
                        if let rvn_ui::motion::Motion::Repeat { motion, .. } = &track.definition {
                            track.elapsed %= motion
                                .validate()
                                .map_err(|message| self.frame_error(&message))?
                                .seconds
                                .ok_or_else(|| self.frame_error("Endless motion has no cycle"))?;
                        }
                    }
                    track
                        .validate()
                        .map_err(|message| self.frame_error(&message))?;
                    plan.outcome.progressed |= seconds > 0.0 || !track.running;
                }
                if !track.running {
                    let waits = &plan.next.scene_execution.as_ref().unwrap().resources.waits;
                    if waits.iter().any(|wait| {
                        wait.key == token.task.key
                            && wait.revision == token.task.revision
                            && wait.target == token.target
                    }) {
                        plan.motions_due.push(token);
                    }
                }
            } else {
                let execution = plan.next.scene_execution.as_mut().unwrap();
                let effect = execution
                    .resources
                    .tasks
                    .iter_mut()
                    .find(|task| {
                        task.key == token.task.key
                            && task.revision == token.task.revision
                            && task.target == token.target
                    })
                    .and_then(|task| task.effect.as_mut())
                    .ok_or_else(|| {
                        self.frame_error("Exact effect task has no logical descriptor")
                    })?;
                effect.clock = effect
                    .clock
                    .advanced(seconds)
                    .map_err(|message| self.frame_error(&message))?;
                effect
                    .validate()
                    .map_err(|message| self.frame_error(&message))?;
                plan.outcome.progressed |= seconds > 0.0;
                if effect.clock.is_due() {
                    plan.effects_due.push(token);
                }
            }
        }

        let execution = plan.next.scene_execution.as_mut().unwrap();
        plan.outcome.progressed |= seconds > 0.0 && execution.game_tasks.timer.is_some();
        let game_due = execution
            .game_tasks
            .advance_timer(seconds)
            .map_err(|message| self.frame_error(&message))?;
        if let Some(task) = game_due {
            plan.game_timer = Some(FrameTimer {
                token: TaskToken {
                    runtime_epoch: self.runtime_epoch,
                    key: TaskKey {
                        owner: TaskOwner::Game,
                        local_id: "timer".into(),
                    },
                    revision: task.revision,
                },
                task,
            });
        }
        if let Some(scene) = &mut execution.active {
            plan.outcome.progressed |= seconds > 0.0 && scene.tasks.timer.is_some();
            if let Some(task) = scene
                .tasks
                .advance_timer(seconds)
                .map_err(|message| self.frame_error(&message))?
            {
                plan.scene_timer = Some(FrameTimer {
                    token: TaskToken {
                        runtime_epoch: self.runtime_epoch,
                        key: TaskKey {
                            owner: TaskOwner::Scene {
                                activation: scene.id,
                            },
                            local_id: "timer".into(),
                        },
                        revision: task.revision,
                    },
                    task,
                });
            }
        }
        Ok(plan)
    }

    fn prepare_frame_canvases(
        &self,
        next: &mut GameState,
        scope: CanvasClockScope,
        seconds: f64,
    ) -> Result<Vec<FrameCanvas>, RuntimeError> {
        if seconds == 0.0 || next.ui.screens.is_empty() {
            return Ok(Vec::new());
        }
        let views = self.describe_interface_structure(next)?;
        self.ui_library.with_locale(self.locale.as_ref())
            .reconcile_canvas_states_from_views(&mut next.ui, &views)
            .map_err(|error| self.eval_err(error, "Frame Canvas hydration"))?;
        if next.scene_execution.is_some() {
            crate::scene_resources::sync_current(next)
                .map_err(|message| self.scene_resource_error(message))?;
        }
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
            let Some(instance) = next
                .ui
                .screens
                .iter()
                .find(|instance| instance.name == view.name)
            else {
                continue;
            };
            let owner = if let Some(role) = instance.host_role {
                TaskOwner::Host {
                    role: role.id().into(),
                }
            } else {
                Self::live_resource(next, ResourceTaskKind::Interface, &view.name)
                    .map(|task| task.key.owner)
                    .unwrap_or_else(|| Self::command_owner(next))
            };
            let include = match scope {
                CanvasClockScope::Story => next
                    .scene_execution
                    .as_ref()
                    .is_some_and(|execution| Self::frame_story_owner(&owner, execution.owner())),
                CanvasClockScope::Host => matches!(owner, TaskOwner::Host { .. }),
            };
            if !include {
                continue;
            }
            let order = instance.order;
            view.root.visit(&mut |component| {
                if component.kind == ComponentKind::Canvas && view.root.available(&component.id) {
                    targets.push((
                        view.name.clone(),
                        component.id.clone(),
                        order,
                        owner.clone(),
                    ));
                }
            });
        }
        let mut result = Vec::new();
        for (screen, element, order, owner) in targets {
            let token = if let Some(execution) = &next.scene_execution {
                let key = crate::scene_resources::resource_key(
                    owner.clone(),
                    ResourceTaskKind::Canvas,
                    &format!("{screen}/{element}"),
                );
                Some(
                    execution
                        .resources
                        .effect_token(self.runtime_epoch, &key, execution.owner())
                        .ok_or_else(|| {
                            self.frame_error("Eligible Canvas has no exact authority")
                        })?,
                )
            } else {
                None
            };
            let entry = next
                .ui
                .screens
                .iter_mut()
                .find(|instance| instance.name == screen && instance.order == order)
                .and_then(|instance| instance.canvas_states.get_mut(&element))
                .ok_or_else(|| self.frame_error("Eligible Canvas has no local state"))?;
            entry.elapsed += seconds;
            if !entry.elapsed.is_finite() || entry.elapsed > 1.0e12 {
                return Err(self.frame_error("Canvas clock overflow"));
            }
            result.push(FrameCanvas {
                screen,
                element,
                order,
                owner,
                token,
            });
        }
        Ok(result)
    }

    fn dispatch_frame_canvas(
        &self,
        next: &mut GameState,
        captured: &FrameCanvas,
        seconds: f64,
        budget: &mut CanvasBudget,
    ) -> Result<bool, RuntimeError> {
        if let Some(token) = &captured.token {
            if !self.frame_exact_resource(next, token, ResourceTaskKind::Canvas) {
                return Ok(false);
            }
        } else if next.scene_execution.is_some()
            || !matches!(captured.owner, TaskOwner::Host { .. })
        {
            return Err(self.frame_error("Canvas authority unexpectedly absent"));
        }
        let Some(instance) =
            next.ui.screens.iter().find(|instance| {
                instance.name == captured.screen && instance.order == captured.order
            })
        else {
            return Ok(false);
        };
        if let TaskOwner::Host { role } = &captured.owner {
            if instance
                .host_role
                .is_none_or(|current| current.id() != role)
            {
                return Ok(false);
            }
        }
        let views = self.describe_interface_structure(next)?;
        let Some(view) = views
            .iter()
            .find(|view| view.name == captured.screen && view.order == captured.order)
        else {
            return Ok(false);
        };
        if views
            .iter()
            .rev()
            .find(|view| view.modal)
            .is_some_and(|modal| (view.layer, view.order) < (modal.layer, modal.order))
            || !view.root.available(&captured.element)
        {
            return Ok(false);
        }
        let Some(component) = view
            .root
            .find(&captured.element)
            .filter(|component| component.kind == ComponentKind::Canvas)
        else {
            return Ok(false);
        };
        if !component.events.contains_key(&ScreenEventKind::Tick) {
            return Ok(false);
        }
        let event = crate::ui::UiInput {
            screen: captured.screen.clone(),
            element: captured.element.clone(),
            kind: ScreenEventKind::Tick,
            value: Some(Value::Dict(std::collections::BTreeMap::from([(
                "dt".into(),
                Value::Float(seconds as f32),
            )]))),
            key: None,
        };
        let commands = self.ui_event_owned_budgeted(next, &event, component,
            &captured.owner, None, None, budget)?;
        self.apply_callback_commands_budgeted(next, commands, budget)?;
        Ok(true)
    }

    fn plan_frame_video_delivery(
        &self,
        next: &mut GameState,
        report: &GameVideoReport,
        budget: &mut CanvasBudget,
        outcome: &mut GameFrameOutcome,
    ) -> Result<bool, RuntimeError> {
        if !self.frame_resource_current(next, &report.token, ResourceTaskKind::Video) {
            return Ok(false);
        }
        let id = &report.token.target.alias;
        let Some(track) = next.videos.tracks.get(id) else {
            return Ok(false);
        };
        if track.finished() {
            return Ok(false);
        }
        // A preceding handler can remove a component without an UiCommand,
        // or park its real interface behind a child with the same name. The
        // captured resident identity, rather than a live alias, is authority.
        if !self.frame_video_target_current(next, id, track)? {
            return Ok(false);
        }
        match &report.feedback {
            GameVideoFeedback::Resume => {
                self.apply_ui_commands_budgeted_owned(
                    next,
                    vec![crate::eval::UiCommand::VideoResume { name: id.clone() }],
                    budget,
                    report.token.task.key.owner.clone(),
                )?;
                Ok(true)
            }
            GameVideoFeedback::Report(feedback) => {
                if matches!(feedback, Feedback::Position { .. }) {
                    return Err(self.frame_error("Position report reached the callback phase"));
                }
                if matches!(feedback, Feedback::End) && track.playback != Playback::Playing {
                    return Ok(false);
                }
                let handler = next
                    .videos
                    .feedback(id, feedback.clone())
                    .map_err(|message| self.frame_error(&message))?;
                if next.videos.tracks[id].finished() {
                    crate::scene_resources::release_waiters(
                        next,
                        &report.token,
                        self.runtime_epoch,
                    )
                    .map_err(|message| self.scene_resource_error(message))?;
                }
                if let Some(handler) = handler {
                    let commands = self.video_handler_budgeted(next, id, &handler, budget)?;
                    self.apply_callback_commands_budgeted(next,commands,budget)?;
                    outcome.callbacks += 1;
                }
                Ok(true)
            }
        }
    }

    fn frame_video_target_current(
        &self,
        state: &GameState,
        id: &str,
        track: &crate::video::Track,
    ) -> Result<bool, RuntimeError> {
        let Some(target) = &track.clip.target else {
            return Ok(true);
        };
        if state.scene_execution.is_some() {
            return Ok(self.video_projection_binding(state, id, track)?.is_some());
        }
        let (screen, element) = target
            .strip_prefix("ui:")
            .and_then(|target| target.split_once('/'))
            .ok_or_else(|| self.frame_error("Invalid embedded video target"))?;
        Ok(self
            .describe_interface_structure(state)?
            .iter()
            .any(|view| view.name == screen && view.root.find(element).is_some()))
    }

    fn frame_timer_current(&self, next: &GameState, captured: &FrameTimer) -> bool {
        if captured.token.runtime_epoch != self.runtime_epoch
            || captured.token.key.local_id != "timer"
        {
            return false;
        }
        let Some(execution) = &next.scene_execution else {
            return false;
        };
        let task = match &captured.token.key.owner {
            TaskOwner::Game => execution.game_tasks.timer.as_ref(),
            TaskOwner::Scene { activation } => execution
                .active
                .as_ref()
                .filter(|scene| scene.id == *activation)
                .and_then(|scene| scene.tasks.timer.as_ref()),
            TaskOwner::Host { .. } => None,
        };
        task.is_some_and(|task| {
            task.revision == captured.token.revision
                && task.duration == captured.task.duration
                && task.action == captured.task.action
                && task.elapsed >= task.duration
        })
    }
}

impl<R:Renderer> Engine<R>{
 fn tick_standalone_game_frame(&mut self,seconds:f64,reports:GameFrameReports,audio:&[crate::audio_playback::AudioReport])->Result<GameFrameOutcome,RuntimeError>{
  if !seconds.is_finite()||!(0.0..=3600.0).contains(&seconds){return Err(self.frame_error("Game delta must be finite and between 0 and 3600 seconds"));}
  if !reports.videos.is_empty(){return Err(self.frame_error("Standalone Game frame cannot adopt a video report; use its actual video route"));}
  if self.state.game_tasks.is_some()&&self.startup.is_some(){return Err(self.frame_error("Startup has not handed its actual tasks to Game"));}
  self.validate_standalone_timer_target(&self.state)?;
  self.consumer_program.validate_emitted(&self.script).and_then(|_|self.consumer_program.validate_state_binding(&self.state)).map_err(|e|self.eval_err(e,"Standalone Game frame source"))?;
  crate::scene_resources::validate_current_resource_state(&self.state).map_err(|m|self.scene_resource_error(m))?;
  let initial_pc=self.state.pc;let initial_stack=self.state.call_stack.clone();
  let mut next=self.state.clone();let mut outcome=GameFrameOutcome::default();let mut legacy_timer=if self.state.game_tasks.is_some(){self.active_timer.clone()}else{None};
  let sprite_tokens=next.game_tasks.as_ref().map(|game|game.resources.active_effects(None).into_iter().filter_map(|t|game.resources.effect_token(self.runtime_epoch,&t.key,None)).collect::<Vec<_>>()).unwrap_or_default();
  for token in &sprite_tokens{
   let effect=next.game_tasks.as_mut().unwrap().resources.tasks.iter_mut().find(|t|t.key==token.task.key&&t.revision==token.task.revision&&t.target==token.target).and_then(|t|t.effect.as_mut()).ok_or_else(||self.frame_error("Exact Game sprite effect disappeared"))?;
   let clock=effect.clock.advanced(seconds).map_err(|m|self.frame_error(&m))?;outcome.progressed|=clock!=effect.clock;effect.clock=clock;effect.validate().map_err(|m|self.frame_error(&m))?;
  }
  let timer_due=if let Some(game)=&mut next.game_tasks{
   let before=game.tasks.timer.as_ref().map(|t|t.elapsed);
   let due=game.tasks.advance_timer(seconds).map_err(|m|self.frame_error(&m))?;
   outcome.progressed|=before!=game.tasks.timer.as_ref().map(|t|t.elapsed);due
  }else{None};
  outcome.progressed|=crate::audio_playback::advance_clocks(&mut next,self.runtime_epoch,crate::audio_playback::AudioClockRoute::Story,seconds).map_err(|m|self.scene_resource_error(m))?;
  let tokens=next.audio_resource_registry().map(|registry|registry.active_effects(None).into_iter().map(|t|crate::scene_resources::ResourceToken{task:crate::scene_tasks::TaskToken{runtime_epoch:self.runtime_epoch,key:t.key.clone(),revision:t.revision},target:t.target.clone()}).collect::<Vec<_>>()).unwrap_or_default();
  for token in &tokens{
   let e=next.audio_resource_registry_mut().unwrap().tasks.iter_mut().find(|t|t.key==token.task.key&&t.revision==token.task.revision&&t.target==token.target).and_then(|t|t.effect.as_mut()).ok_or_else(||self.frame_error("Exact Game audio effect disappeared"))?;
   let clock=e.clock.advanced(seconds).map_err(|m|self.frame_error(&m))?;outcome.progressed|=clock!=e.clock;e.clock=clock;e.validate().map_err(|m|self.frame_error(&m))?;
  }
  outcome.progressed|=self.apply_audio_deliveries(&mut next,audio)?;
  // Consume only the timer captured at frame entry. Any task created after
  // this callback waits for the next actual frame, as in the narrative path.
  if let Some(timer)=timer_due{
   if next.game_tasks.as_ref().and_then(|g|g.tasks.timer.as_ref()).is_some_and(|t|t.revision==timer.revision&&t.action==timer.action&&t.duration==timer.duration&&t.elapsed>=t.duration){
    let historical=next.game_tasks.as_ref().unwrap().historical_timer;
    next.game_tasks.as_mut().unwrap().tasks.timer=None;next.game_tasks.as_mut().unwrap().historical_timer=false;
    next=self.plan_standalone_timer_action(next,&timer.action,historical)?;
    outcome.callbacks+=1;outcome.progressed=true;
   }
  }
  // A nonportable raw timer keeps the existing Engine slot, exclusively.
  // This same frame transaction advances it with its historical f32 clock;
  // no second Bevy timer system or scheduler is introduced.
  if let Some((duration,action,elapsed))=&mut legacy_timer{
   let before=elapsed.to_bits();*elapsed+=seconds as f32;outcome.progressed|=elapsed.to_bits()!=before;
   if *elapsed>=*duration{let action=action.clone();legacy_timer=None;next=self.plan_standalone_timer_action(next,&action,true)?;outcome.callbacks+=1;outcome.progressed=true;}
  }
  outcome.progressed|=crate::audio_playback::complete_due_clocks(&mut next,self.runtime_epoch,crate::audio_playback::AudioClockRoute::Story).map_err(|m|self.scene_resource_error(m))?;
  for token in tokens{outcome.progressed|=crate::scene_resources::complete_due_effect(&mut next,&token,self.runtime_epoch).map_err(|m|self.scene_resource_error(m))?.is_some();}
  for token in sprite_tokens{outcome.progressed|=crate::scene_resources::complete_due_effect(&mut next,&token,self.runtime_epoch).map_err(|m|self.scene_resource_error(m))?.is_some();}
  outcome.scene_changed=initial_pc!=next.pc||initial_stack!=next.call_stack;
  if outcome.progressed{self.commit_audio_state(next)?;if self.state.game_tasks.is_some(){self.active_timer=legacy_timer;}outcome.published=true;}Ok(outcome)
 }
}
