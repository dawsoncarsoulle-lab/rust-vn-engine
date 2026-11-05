//! An admitted bootstrap owns its real Engine until initialization completes.
//! Its cursor is derived from accepted Init roots, never a gameplay sentinel.
use super::*;
use rvn_parser::ParsedProgram;
use serde::{Deserialize, Serialize};
#[path="engine_startup_step.rs"]
mod step;
#[path="engine_startup_cursor.rs"]
mod cursor;
#[path="engine_startup_imagemap_route_0994.rs"]
mod imagemap_route;
#[path="engine_startup_imagemap_0996.rs"]
mod imagemap;
pub use imagemap::{StartupInputBatchSink,StartupImagemapSource};
#[path="engine_startup_loops.rs"]
mod loops;
#[path="engine_startup_native_loops.rs"]
mod native_loops;
#[path="engine_startup_runtime.rs"]
mod runtime;
#[path="engine_startup_wait.rs"]
mod wait;
pub use wait::StartupVideoClockSource;
#[path="engine_startup_feedback.rs"]
mod feedback;
#[path="engine_startup_terminal.rs"]
mod terminal;
#[path="engine_startup_driver.rs"]
mod driver;
pub use terminal::{PreparedStartupVideoContinuation,AcceptedStartupVideoTerminal};
pub use feedback::{StartupVideoReport,PreparedStartupVideoFeedback,StartupVideoFeedbackGuard,CommittedStartupVideoFeedback,AcceptedStartupVideoReport,StartupVideoReportKind,MAX_STARTUP_VIDEO_REPORTS,MAX_STARTUP_VIDEO_METADATA_BYTES};
#[path="engine_startup_video_commands.rs"]
pub(in crate::engine) mod video_commands;
pub use video_commands::{StartupVideoCommand,StartupVideoCommandKind};
pub use step::{PreparedStartupStep,StartupStepCommitGuard,StartupPublicationSource,StartupBatchSink,StartupHostEffects,StartupHostStepGuard,CommittedStartupStep,StartupObservationSource};
#[cfg(test)]
#[path = "engine_startup_tests.rs"]
mod tests;

#[derive(Clone, Debug)]
pub(crate) struct InitRoot {
    pub(crate) pc: usize,
    source_ordinal: usize,
    definition_root_ordinal: usize,
}
#[derive(Clone)]
pub(crate) struct StartupState {
    pub(crate) roots: Vec<InitRoot>,
    root: usize,
    body: usize,
    nested:Vec<wire::Route>,
    exit_pc: usize,
    // The real statement at this route has executed once and awaits media.
    // Its source cursor remains at that authored Wait until release.
    waiting_video:Option<String>,
    imagemaps:Vec<imagemap::PortableStartupImagemap>,
    loops:Vec<PortableStartupLoop>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableStartup {
    root: usize,
    body: usize,
    source_ordinal: usize,
    definition_root_ordinal: usize,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    nested:Vec<wire::Route>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    waiting:Option<PortableStartupWait>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    callback:Option<terminal::PortableStartupVideoCallback>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    loops:Vec<PortableStartupLoop>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    imagemaps:Vec<imagemap::PortableStartupImagemap>,
}
/// Inert saved data. Only the actual Init owner can admit a source-bound frame.
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableStartupLoop {
    header:Vec<wire::Route>,
    header_origin:crate::program::InstructionOrigin,
    // The actual completed pure input root(s), retained as checkpoint data.
    // Their executable roots are omitted by PortableRecord's existing serde.
    gather:Vec<crate::eval::debugger_eval::PortableRecord>,
    visits:usize,
    state:PortableStartupLoopState,
    // Optional inert source-bound history of an actually reached v3 Break.
    // Existing saved loops omit this field byte-for-byte.
    #[serde(default,skip_serializing_if="Option::is_none")]
    native_break:Option<NativeStartupBreak>,
}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeStartupBreak {
    route:Vec<wire::Route>,
    origin:crate::program::InstructionOrigin,
    visits:usize,
}
#[derive(Clone,Debug,PartialEq,Eq)]
struct StartupLoopVisit { header:Vec<wire::Route>, visits:usize }
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(tag="kind",deny_unknown_fields)]
pub(crate) enum PortableStartupLoopState {
    While,
    ForEach { items:Vec<Value>, next:usize },
    ForRange { first:i64, count:usize, next:usize },
    // Additive named data only for explicit source v3. Historical variants and
    // their serialized fields remain byte-identical when no v3 loop is active.
    DemandV1 { cursor:crate::native_loop_cursor::DemandLoopCursor },
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",deny_unknown_fields)]
pub(crate) enum PortableStartupWait{WaitingVideo{name:String},WaitingImagemap}
impl PortableStartup{
    pub(crate) fn waiting_video_name(&self)->Option<&str>{match &self.waiting{Some(PortableStartupWait::WaitingVideo{name})=>Some(name),Some(PortableStartupWait::WaitingImagemap)|None=>None}}
    fn pending_video_wait_name(&self)->Option<&str>{self.waiting_video_name().or_else(||self.callback.as_ref().map(|callback|callback.waiting_name.as_str()))}
    pub(crate) fn has_video_callback(&self)->bool{self.callback.is_some()}
    pub(crate) fn has_imagemap_wait(&self)->bool{matches!(self.waiting,Some(PortableStartupWait::WaitingImagemap))}
    pub(crate) fn validate_callback_wire(&self)->Result<(),crate::save::SaveError>{
        if let Some(callback)=&self.callback{
            if self.waiting.is_some(){return Err(crate::save::SaveError::Incompatible("Saved startup cannot combine waiting and callback phases".into()));}
            callback.validate()?;
        }Ok(())
    }
}

/// Issued by an owned pending bootstrap only. Client data cannot construct it.
#[derive(Clone)]
pub struct StartupOperation {
    root: usize,
    body: usize,
    route:Vec<wire::Route>,
    pc: usize,
    instance: u64,
    epoch: u64,
    pub(super) origin: crate::program::InstructionOrigin,
    // Only actual waiting-owner admission can install a terminal intent.
    video_feedback:Option<terminal::AdmittedStartupVideoFeedback>,
    loop_visits:Vec<StartupLoopVisit>,
}
impl StartupOperation {
    pub(super) fn entry_site(&self) -> wire::Site {
        wire::Site {
            function: None,
            route: self.route.clone(),
            depth: 0,
            statement: true,
        }
    }
}

/// No mutable Engine/gameplay/renderer handle is exposed while Init is pending.
pub struct EngineStartup<R: Renderer> {
    engine: Engine<R>,
    program: std::sync::Arc<ParsedProgram>,
    session: Option<DebugSession>,
    breakpoints: Vec<wire::Breakpoint>,
    watches: Vec<wire::Watch>,
    // Derived solely from an accepted real publication, never a host DTO.
    // Reading it cannot warm Engine's source-authenticated interface cache.
    video_projection:Vec<crate::video::VideoView>,
}
/// Holds the actual Engine guard while its host maintains resource/audio fences.
pub struct StartupCommitGuard<'a, R: Renderer> {
    inner: Save15CommitGuard<'a, R>,
    session: &'a mut Option<DebugSession>,
    breakpoints: &'a mut Vec<wire::Breakpoint>,
    watches: &'a mut Vec<wire::Watch>,
    video_projection:&'a mut Vec<crate::video::VideoView>,
    prepared_videos:Vec<crate::video::VideoView>,
    waiting_configuration:Option<(Vec<wire::Breakpoint>,Vec<wire::Watch>)>,
}
impl<R: Renderer> StartupCommitGuard<'_, R> {
    pub fn render_batch(&self) -> &crate::renderer::SceneRenderBatch {
        self.inner.render_batch()
    }
    fn install(
        session: Option<DebugSession>,
        waiting_configuration:Option<(Vec<wire::Breakpoint>,Vec<wire::Watch>)>,
        target: &mut Option<DebugSession>,
        breakpoints: &mut Vec<wire::Breakpoint>,
        watches: &mut Vec<wire::Watch>,
    ) {
        if let Some(session)=session{
            *breakpoints = session.breakpoints.clone();
            *watches = session.watches.clone();
            *target = Some(session);
        }else{
            let(next_breakpoints,next_watches)=waiting_configuration.expect("Admitted waiting Init owns its validated configuration");
            *breakpoints=next_breakpoints;*watches=next_watches;*target=None;
        }
    }
    pub fn commit(self) {
        let session = self.inner.commit();
        Self::install(session,self.waiting_configuration,self.session,self.breakpoints,self.watches);
        *self.video_projection=self.prepared_videos;
    }
    pub fn commit_with_audio_checkpoint(
        self,
        snapshot: crate::audio_checkpoint::AudioCheckpointSnapshot,
    ) -> Result<(), crate::save::SaveError> {
        let session = self
            .inner
            .commit_with_audio_checkpoint(snapshot)?;
        Self::install(session,self.waiting_configuration,self.session,self.breakpoints,self.watches);
        *self.video_projection=self.prepared_videos;
        Ok(())
    }
}

impl StartupState {
    pub(super) fn is_waiting_video(&self)->bool{self.waiting_video.is_some()}
    fn from_program<R: Renderer>(
        engine: &Engine<R>,
        program: &ParsedProgram,
    ) -> Result<Self, RuntimeError> {
        let map = engine.functions.debug_source_map.as_ref().ok_or_else(|| {
            engine.scene_resource_error("Startup has no accepted source map".into())
        })?;
        let mut roots = Vec::new();
        let mut exit_pc = 0;
        for (definition_root_ordinal, (statement, origin)) in
            program.script.iter().zip(&program.origins).enumerate()
        {
            if !matches!(statement, Statement::Init { .. }) {
                continue;
            }
            if roots.len() >= 100_000 {
                return Err(engine.scene_resource_error("Initializer count budget".into()));
            }
            let source_ordinal = program
                .sources
                .iter()
                .position(|source| source.path == origin.source)
                .ok_or_else(|| {
                    engine.scene_resource_error("Initializer source ordinal absent".into())
                })?;
            let candidates = engine
                .script
                .iter()
                .enumerate()
                .filter(|(pc, actual)| {
                    *actual == statement
                        && map.origins.get(*pc).is_some_and(|site| {
                            site.source == origin.source
                                && site.span.as_ref() == Some(&origin.span.range)
                        })
                })
                .map(|(pc, _)| pc)
                .collect::<Vec<_>>();
            let [pc] = candidates.as_slice() else {
                return Err(engine.scene_resource_error(
                    "Initializer has no unique genuine compiled root".into(),
                ));
            };
            if *pc >= engine.init_scan_end {
                return Err(engine
                    .scene_resource_error("Initializer is outside actual bootstrap scan".into()));
            }
            roots.push(InitRoot {
                pc: *pc,
                source_ordinal,
                definition_root_ordinal,
            });
            if exit_pc == *pc {
                exit_pc += 1;
            }
        }
        if engine.narrative.is_some() {
            exit_pc = 0;
        }
        Ok(Self {
            roots,
            root: 0,
            body: 0,
            nested:Vec::new(),
            exit_pc,
            waiting_video:None,
            imagemaps:Vec::new(),
            loops:Vec::new(),
        })
    }
    fn current(&self) -> Option<&InitRoot> {
        self.roots.get(self.root)
    }
    fn portable(&self) -> Option<PortableStartup> {
        self.current().map(|root| PortableStartup {
            root: self.root,
            body: self.body,
            source_ordinal: root.source_ordinal,
            definition_root_ordinal: root.definition_root_ordinal,
            nested:self.nested.clone(),
            waiting:if self.waiting_imagemap(){Some(PortableStartupWait::WaitingImagemap)}else{self.waiting_video.as_ref().map(|name|PortableStartupWait::WaitingVideo{name:name.clone()})},
            callback:None,
            loops:self.loops.clone(),
            imagemaps:self.imagemaps.clone(),
        })
    }
    pub(super) fn restore(
        &mut self,
        portable: &PortableStartup,
        script: &[Statement],
    ) -> Result<(), String> {
        let root = self
            .roots
            .get(portable.root)
            .ok_or("Saved startup root ordinal is outside actual Init table")?;
        let Statement::Init { body } = script
            .get(root.pc)
            .ok_or("Saved startup compiled root absent")?
        else {
            return Err("Saved startup root is not an actual Init".into());
        };
        if root.source_ordinal != portable.source_ordinal
            || root.definition_root_ordinal != portable.definition_root_ordinal
            || portable.body >= body.len()
        {
            return Err("Saved startup source/body ordinals differ from actual Init".into());
        }
        self.root = portable.root;
        self.body = portable.body;
        if portable.nested.len()>128{return Err("Saved startup nested route budget".into());}
        self.nested=portable.nested.clone();
        if portable.imagemaps.len()>64{return Err("Saved Init input stack exceeds its actual source route bound".into());}
        self.imagemaps=portable.imagemaps.clone();
        if self.waiting_imagemap()!=matches!(portable.waiting,Some(PortableStartupWait::WaitingImagemap)){return Err("Saved Init input wait flag differs from its retained source continuation".into());}
        self.statement(script)?;
        self.restore_loop_frames(portable,script)?;
        portable.validate_callback_wire().map_err(|error|error.to_string())?;
        self.waiting_video=match portable.pending_video_wait_name(){
            Some(name)=>{
                if name.len()>128||!rvn_ui::composition::valid_name(name){return Err("Saved initializer wait player identity exceeds its bounds".into());}
                let mut statement=self.statement(script)?;
                while let Statement::ScheduledConsumer{body,..}=statement{
                    let [child]=body.as_slice()else{return Err("Saved initializer wait has no unique actual consumer body".into());};statement=child;
                }
                if !matches!(statement,Statement::VideoWait{..}){return Err("Saved initializer wait route is not an actual VideoWait".into());}
                Some(name.to_owned())
            },None=>None,
        };
        Ok(())
    }
    fn skip_empty(&mut self, script: &[Statement]) -> Result<(), String> {
        while let Some(root) = self.current() {
            let Some(Statement::Init { body }) = script.get(root.pc) else {
                return Err("Startup root changed kind".into());
            };
            if self.body < body.len() {
                return Ok(());
            }
            self.root += 1;
            self.body = 0;
            self.nested.clear();
        }
        Ok(())
    }
}

impl<R: Renderer> Engine<R> {
    /// Validate the actual source closure and compiler overlay before any Init.
    pub fn prepare_debugger_startup(
        program: ParsedProgram,
        renderer: R,
        rollback_depth: usize,
        persistent_vars: HashMap<String, Value>,
        text_context: TextRuntimeContext,
        overlay: Option<(crate::debugger_provenance::Overlay, u64)>,
    ) -> Result<EngineStartup<R>, RuntimeError> {
        Self::prepare_owned_startup(std::sync::Arc::new(program),renderer,rollback_depth,persistent_vars,text_context,overlay,(0,0,0))
    }
    pub(in crate::engine) fn prepare_owned_startup(
        program: std::sync::Arc<ParsedProgram>,
        renderer: R,
        rollback_depth: usize,
        persistent_vars: HashMap<String, Value>,
        text_context: TextRuntimeContext,
        overlay: Option<(crate::debugger_provenance::Overlay, u64)>,
        generations:(u64,u64,u64),
    ) -> Result<EngineStartup<R>, RuntimeError> {
        Self::prepare_owned_startup_in(program,renderer,rollback_depth,persistent_vars,text_context,overlay,generations,None)
    }
    pub(in crate::engine) fn prepare_owned_startup_in(
        program: std::sync::Arc<ParsedProgram>,
        renderer: R,
        rollback_depth: usize,
        persistent_vars: HashMap<String, Value>,
        text_context: TextRuntimeContext,
        overlay: Option<(crate::debugger_provenance::Overlay, u64)>,
        generations:(u64,u64,u64),
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>,
    ) -> Result<EngineStartup<R>, RuntimeError> {
        Self::prepare_owned_startup_enum_in1025(program,renderer,rollback_depth,persistent_vars,text_context,overlay,generations,startup_input,Default::default())
    }
    pub(in crate::engine) fn prepare_owned_startup_enum_in1025(
        program:std::sync::Arc<ParsedProgram>,renderer:R,rollback_depth:usize,
        persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,
        overlay:Option<(crate::debugger_provenance::Overlay,u64)>,generations:(u64,u64,u64),
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>,
        enums:std::sync::Arc<crate::enum_runtime_resources1025::EnumAssets1025>,
    )->Result<EngineStartup<R>,RuntimeError>{
        Self::prepare_owned_startup_library_enum_in1027(program,renderer,rollback_depth,persistent_vars,text_context,overlay,generations,startup_input,enums,None)
    }
    pub(in crate::engine) fn prepare_owned_startup_library_enum_in1027(
        program:std::sync::Arc<ParsedProgram>,renderer:R,rollback_depth:usize,
        persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,
        overlay:Option<(crate::debugger_provenance::Overlay,u64)>,generations:(u64,u64,u64),
        startup_input:Option<std::sync::Arc<crate::consumer_schedule::StartupInputProgram>>,
        enums:std::sync::Arc<crate::enum_runtime_resources1025::EnumAssets1025>,
        libraries:Option<FunctionLibrary>,
    )->Result<EngineStartup<R>,RuntimeError>{
        for value in persistent_vars.values(){crate::enum_runtime_resources1025::validate_value(&enums,value)
            .map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;}

        crate::eval::validate_enum_program_literals1027(&program.script,&enums)
            .map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        crate::save::consumer_checkpoint::preflight_program(&program).map_err(|e| {
            RuntimeError::no_stmt(
                crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(
                    e.to_string(),
                )),
                0,
            )
        })?;
        let invalid = |message: &str| {
            RuntimeError::no_stmt(
                crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(
                    message.into(),
                )),
                0,
            )
        };
        let mut bytes = 0usize;
        for source in &program.sources {
            bytes = bytes
                .checked_add(source.source.len())
                .filter(|n| *n <= crate::debugger_provenance::MAX_BYTES)
                .ok_or_else(|| {
                    invalid("Startup frozen UTF-8 bytes exceed budget before cloning")
                })?;
            if source
                .path
                .as_ref()
                .is_some_and(|p| p.as_os_str().len() > 4096)
            {
                return Err(invalid("Startup source path budget"));
            }
        }
        let mut nodes = 0usize;
        let mut pending = program
            .origins
            .iter()
            .map(|origin| (&origin.span, 0usize))
            .collect::<Vec<_>>();
        while let Some((span, depth)) = pending.pop() {
            nodes += 1;
            if depth > 128 || nodes > 100_000 {
                return Err(invalid("Startup actual source span depth/count budget"));
            }
            pending.extend(span.children.iter().map(|child| (child, depth + 1)));
        }
        let map = crate::debugger_provenance::SourceMap::from_program(&program).map_err(|e| {
            RuntimeError::no_stmt(
                crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(e)),
                0,
            )
        })?;
        // Overlay admission also precedes construction. A forged graph overlay
        // cannot run Init and only then be refused by the debugger installer.
        let map = if let Some((overlay, nonce)) = overlay {
            overlay.into_verified_map(&map, nonce).map_err(|e| {
                RuntimeError::no_stmt(
                    crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(e)),
                    0,
                )
            })?
        } else {
            map
        };
        let mut engine = Self::from_program_bootstrap_libraries1023_inputs(
            program.as_ref().clone(),
            renderer,
            rollback_depth,
            persistent_vars,
            text_context,
            true,
            None,
            startup_input,
            libraries,
        )?;
        // Install the actual loaded nominal resources before StartupState or
        // prepare_session can evaluate any Init expression.
        engine.functions.enum_assets1025=enums;
        engine.runtime_epoch=generations.0;engine.interface_epoch=generations.1;engine.video_epoch=generations.2;
        engine.frozen_startup_program=Some(std::sync::Arc::clone(&program));
        engine.functions.debug_origins = map.sites.clone();
        engine.functions.debug_source_map = Some(map);
        engine.functions.debug_source_map_installed = true;
        let mut state = StartupState::from_program(&engine, &program)?;
        state
            .skip_empty(&engine.script)
            .map_err(|e| engine.scene_resource_error(e))?;
        engine.state.pc = state.current().map_or(state.exit_pc, |root| root.pc);
        engine.sync_narrative_position();
        if state.current().is_some() {
            engine.startup = Some(state);
        }
        let mut pending = EngineStartup {
            engine,
            program,
            session: None,
            breakpoints: Vec::new(),
            watches: Vec::new(),
            video_projection:Vec::new(),
        };
        pending.prepare_session()?;
        Ok(pending)
    }
    pub(super) fn startup_operation(&self) -> Result<StartupOperation, RuntimeError> {
        let state = self.startup.as_ref().ok_or_else(|| {
            self.scene_resource_error("Initialization is already complete".into())
        })?;
        let root = state.current().ok_or_else(|| {
            self.scene_resource_error("Initialization cursor has no real root".into())
        })?;
        if self.state.pc != root.pc {
            return Err(self.scene_resource_error(
                "Initialization cursor differs from actual prepared root".into(),
            ));
        }
        let route = state.route();
        let origin=self.startup_origin_for_route(&route)?;
        Ok(StartupOperation {
            root: state.root,
            body: state.body,
            route,
            pc: root.pc,
            instance: self.instance_id(),
            epoch: self.runtime_epoch(),
            origin,
            video_feedback:None,
            loop_visits:state.loop_visits(),
        })
    }
    pub(super) fn validate_startup_operation(
        &self,
        operation: &StartupOperation,
    ) -> Result<(), RuntimeError> {
        let actual = self.startup_operation()?;
        if operation.root != actual.root
            || operation.body != actual.body
            || operation.route != actual.route
            || operation.pc != actual.pc
            || operation.instance != actual.instance
            || operation.epoch != actual.epoch
            || operation.origin != actual.origin
            || operation.loop_visits != actual.loop_visits
        {
            return Err(self.scene_resource_error(
                "Startup command belongs to a stale source/owner/cursor".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn initializer_consumer_route(
        &self,
        statement: &Statement,
    ) -> Result<Vec<wire::Route>, RuntimeError> {
        let Statement::ScheduledConsumer {
            scope, consumer, ..
        } = statement
        else {
            unreachable!()
        };
        let state = self.startup.as_ref().ok_or_else(|| {
            self.scene_resource_error("Initializer consumer has no startup owner".into())
        })?;
        let root = state
            .current()
            .ok_or_else(|| self.scene_resource_error("Initializer consumer root absent".into()))?;
        let mut found = None;
        for record in &self.functions.debug_origins {
            if record.origin.scheduled_step.as_ref().is_some_and(|step| {
                step.definition_root_ordinal == root.definition_root_ordinal
                    && step.source_ordinal == root.source_ordinal
                    && step.plan_scope == *scope
                    && step.consumer_identity == *consumer
            }) {
                let mut route = record.route.clone();
                if !matches!(route.pop(), Some(wire::Route::Producer(_))) {
                    return Err(self.scene_resource_error(
                        "Initializer consumer source has no producer route".into(),
                    ));
                }
                if found.as_ref().is_some_and(|old| old != &route) {
                    return Err(self.scene_resource_error(
                        "Initializer consumer source route is ambiguous".into(),
                    ));
                }
                found = Some(route);
            }
        }
        if found.is_none() {
            fn at<'a>(root: &'a Statement, route: &[wire::Route]) -> Option<&'a Statement> {
                let Statement::Init { body } = root else {
                    return None;
                };
                let mut body = body.as_slice();
                let mut route = route;
                loop {
                    let (wire::Route::Statement(index), tail) = route.split_first()? else {
                        return None;
                    };
                    let node = body.get(*index)?;
                    if tail.is_empty() {
                        return Some(node);
                    }
                    let (part, next) = tail.split_first()?;
                    body = match (node, part) {
                        (
                            Statement::ScheduledConsumer { body, .. }
                            | Statement::While { body, .. }
                            | Statement::ForEach { body, .. }
                            | Statement::ForLoopWithBreak { body, .. },
                            wire::Route::Body,
                        ) => body,
                        (Statement::If { then_branch, .. }, wire::Route::Then) => then_branch,
                        (Statement::If { else_branch, .. }, wire::Route::Else) => else_branch,
                        (Statement::Sequence { branches }, wire::Route::Branch(index)) => {
                            branches.get(*index)?
                        }
                        _ => return None,
                    };
                    route = next;
                }
            }
            let map = self.functions.debug_source_map.as_ref().ok_or_else(|| {
                self.scene_resource_error("Initializer source table absent".into())
            })?;
            let declaration = &map.origins[root.pc];
            for record in &map.sites {
                if record.scope.is_empty()
                    && record.origin.scheduled_step.is_none()
                    && record.declaration.source == declaration.source
                    && record.declaration.span == declaration.span
                    && at(&self.script[root.pc], &record.route) == Some(statement)
                {
                    if found.as_ref().is_some_and(|old| old != &record.route) {
                        return Err(self.scene_resource_error(
                            "Producer-free initializer consumer source route is ambiguous".into(),
                        ));
                    }
                    found = Some(record.route.clone());
                }
            }
        }
        // A plan with no producers still has its genuine statement boundary.
        Ok(found.unwrap_or_else(|| vec![wire::Route::Statement(state.body)]))
    }
    pub(super) fn execute_startup_statement(
        &mut self,
        operation: StartupOperation,
    ) -> Result<(), RuntimeError> {
        self.validate_startup_operation(&operation)?;
        if operation.video_feedback.is_some(){return self.execute_startup_video_feedback(operation);}
        let probe = self.functions.debug_probe.clone().ok_or_else(|| {
            self.scene_resource_error(
                "Startup execution requires its owned debugger continuation".into(),
            )
        })?;
        let mut candidate = self.debugger_candidate(probe.clone());
        let effects=std::rc::Rc::new(std::cell::RefCell::new(LegacyIntents::default()));
        candidate.renderer.2=Some(effects.clone());
        candidate.initializing_consumers = true;
        candidate.consumer_init_pc = Some(operation.pc);
        let statement=candidate.startup.as_ref().unwrap().statement(&candidate.script).map_err(|e|self.scene_resource_error(e))?.clone();
        let explicit_video_wait=matches!(Self::consumer_leaf(Some(&statement)),Some(Statement::VideoWait{..}));
        let computation_only=Self::is_owned_startup_control(&statement)||matches!(&statement,Statement::If{..})||matches!(&statement,Statement::ScheduledConsumer{body,..}if matches!(body.as_slice(),[Statement::If{..}]));
        let cursor_before=candidate.startup.as_ref().unwrap().position_key();
        Probe::initializer_route(&probe, operation.route.clone(), || {
            if !matches!(statement,Statement::ScheduledConsumer{..})&&Self::is_owned_startup_control(&statement){candidate.exec_startup_control(statement)}else{candidate.exec_silent(statement)}
        })
        .map_err(|e| self.eval_err(e, "Initializer source route"))??;
        candidate.initializing_consumers = false;
        candidate.consumer_init_pc = None;
        effects.borrow().validate().map_err(|error|self.scene_resource_error(error))?;
        let mut effects=std::mem::take(&mut *effects.borrow_mut()).into_startup_effects();
        let startup = candidate
            .startup
            .as_mut()
            .ok_or_else(|| self.scene_resource_error("Startup cursor lost its owner".into()))?;
        if explicit_video_wait&&candidate.state.videos.waiting.is_some(){
            startup.waiting_video=candidate.state.videos.waiting.clone();
        }else if !startup.waiting_imagemap()&&startup.position_key()==cursor_before{
            startup.advance(&candidate.script).map_err(|e|self.scene_resource_error(e))?;
        }
        candidate.state.pc = startup.current().map_or(startup.exit_pc, |root| root.pc);
        if startup.current().is_none() {
            candidate.startup = None;
        }
        candidate.sync_narrative_position();
        if explicit_video_wait&&candidate.state.videos.waiting.is_some(){
            // Reconcile through the normal registry at the genuine retained
            // Init position, after the statement's temporary evaluator PC.
            crate::scene_resources::sync_current(&mut candidate.state).map_err(|error|self.scene_resource_error(error))?;
        }
        if computation_only{
            // A condition/gather changes only its real logical state/cursor.
            // It neither rebuilds bindings nor touches renderer/media history.
            let(state,startup,persistent,input,timer,host_random,history,locale)=(candidate.state.clone(),candidate.startup.clone(),candidate.persistent_vars.clone(),candidate.input_state.clone(),candidate.active_timer.clone(),candidate.host_random.clone(),candidate.history.debugger_copy(),candidate.locale.take());
            drop(candidate);self.state=state;self.startup=startup;self.persistent_vars=persistent;self.input_state=input;self.active_timer=timer;self.host_random=host_random;self.history=history;self.locale=locale;return Ok(());
        }
        let mut batch = candidate.build_scene_batch(
            candidate.state.clone(),
            candidate.runtime_epoch,
            candidate.interface_epoch,
            candidate.video_epoch,
        )?;
        batch.display=candidate.startup_imagemap_interaction()?;
        batch.previous = self.state.clone();
        self.renderer
            .validate_scene_batch(&batch)
            .map_err(|e| self.scene_resource_error(e))?;
        // No fallible work follows live mutation. Preparation uses the existing
        // PlanningRenderer; publication consumes already-completed computations.
        let (startup, persistent, input, timer, host_random, history, locale) = (
            candidate.startup.clone(),
            candidate.persistent_vars.clone(),
            candidate.input_state.clone(),
            candidate.active_timer.clone(),
            candidate.host_random.clone(),
            candidate.history.debugger_copy(),
            candidate.locale.take(),
        );
        drop(candidate);
        self.state = batch.state.clone();
        // The real published views carry these candidate epochs. Keeping the
        // outgoing legacy video epoch would revoke our own accepted views.
        self.runtime_epoch=batch.runtime_epoch;
        self.interface_epoch=batch.interface_epoch;
        self.video_epoch=batch.video_epoch;
        self.startup = startup;
        self.persistent_vars = persistent;
        self.input_state = input;
        self.active_timer = timer;
        self.host_random = host_random;
        self.history = history;
        self.locale = locale;
        self.interface_view_cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        self.renderer.publish_scene_batch(batch);
        effects.publish(&mut self.renderer);
        Ok(())
    }
    pub(super) fn portable_startup(&self) -> Option<PortableStartup> {
        self.startup.as_ref().and_then(StartupState::portable)
    }
}

impl<R: Renderer> EngineStartup<R> {
    pub(in crate::engine) fn with_widget_classes(mut self,classes:std::sync::Arc<crate::widget_class_source_0950::AdmittedWidgetClasses>)->Result<Self,RuntimeError>{
        if classes.is_empty()&&self.engine.widget_classes.is_empty(){return Ok(self);}
        if !classes.matches_program(&self.program).map_err(|s|self.engine.scene_resource_error(s))?{return Err(self.engine.scene_resource_error("Widget classes differ from actual Startup program".into()));}
        self.engine.widget_classes=classes;
        self.engine.functions.widget_classes=self.engine.widget_classes.clone();
        self.engine.functions.widget_macro_sites=std::sync::Arc::new(
            crate::widget_class_source_0950::standard_macros::MacroSites::admit(&self.engine.widget_classes,&self.program)
                .map_err(|message|self.engine.scene_resource_error(message))?);
        if self.engine.narrative.is_none()&&!self.engine.widget_classes.is_empty(){self.engine.state.ui.instance_registry.get_or_insert_with(Default::default);}
        self.session=None;self.prepare_session()?;Ok(self)
    }
    fn prepare_session(&mut self) -> Result<(), RuntimeError> {
        if self.engine.startup.as_ref().is_some_and(StartupState::waiting_imagemap){self.session=None;return Ok(());}
        if self.engine.startup.as_ref().is_some_and(|startup|startup.waiting_video.is_some()){
            self.engine.validate_startup_video_wait()?;
            self.session=None;return Ok(());
        }
        if self.engine.startup.is_none() {
            self.session = None;
            return Ok(());
        }
        let operation = self.engine.startup_operation()?;
        let mut session = self
            .engine
            .debug_begin(DebugOperation::Startup(operation), self.breakpoints.clone())?;
        session
            .set_watches(self.watches.clone())
            .map_err(|e| self.engine.scene_resource_error(e))?;
        self.session = Some(session);
        Ok(())
    }
    pub fn program(&self) -> &ParsedProgram {
        &self.program
    }
    pub fn state(&self) -> &GameState {
        &self.engine.state
    }
    pub fn video_views(&self)->Result<Vec<crate::video::VideoView>,RuntimeError>{
        Ok(self.video_projection.clone())
    }
    pub fn is_complete(&self) -> bool {
        self.engine.startup.is_none()
    }
    pub fn snapshot(&self) -> wire::Snapshot {
        if self.waiting_for_imagemap(){return self.imagemap_wait_snapshot();}
        if self.waiting_for_video(){return self.wait_snapshot();}
        self.session
            .as_ref()
            .map(|s| self.engine.debug_session_snapshot(s))
            .unwrap_or_else(|| {
                self.engine
                    .debug_current_snapshot(&self.watches)
                    .expect("admitted bounded watches")
            })
    }
    pub fn site_page(&self, offset: usize) -> Result<wire::SitePage, RuntimeError> {
        self.engine.debug_site_page(offset)
    }
    pub fn configure(
        &mut self,
        breakpoints: Vec<wire::Breakpoint>,
        watches: Vec<wire::Watch>,
    ) -> Result<(), RuntimeError> {
        if self.waiting_for_video()||self.waiting_for_imagemap(){
            if self.waiting_for_video(){self.engine.validate_startup_video_wait()?;}else{self.engine.validate_startup_imagemap_wait()?;}
            self.engine.validate_debug_breakpoints(&breakpoints)?;
            if watches.len()>wire::MAX_WATCHES||watches.iter().any(|watch|!rvn_parser::is_variable_binding_name(&watch.name)||watch.name.len()>4096){return Err(self.engine.scene_resource_error("Invalid or oversized debugger watches".into()));}
            self.breakpoints=breakpoints;self.watches=watches;return Ok(());
        }
        let session = self.session.as_mut().ok_or_else(|| {
            self.engine
                .scene_resource_error("Startup is already complete".into())
        })?;
        self.engine
            .configure_debug_session(session, breakpoints.clone(), watches.clone())?;
        self.breakpoints = breakpoints;
        self.watches = watches;
        Ok(())
    }
    pub fn step(&mut self, kind: StepKind) -> Result<DebugProgress, RuntimeError> {
        let prepared=self.prepare_step_boxed(kind)?;
        Ok(self.begin_prepared_step_commit(*prepared)?.commit())
    }
    fn advance_step(&mut self, kind:StepKind)->Result<DebugProgress,RuntimeError>{
        let mut transitions=0usize;let mut work=0usize;
        loop{
        let logical=self.engine.startup.as_ref().and_then(|startup|startup.statement(&self.engine.script).ok()).is_some_and(|statement|matches!(statement,Statement::If{..})||matches!(statement,Statement::ScheduledConsumer{body,..}if matches!(body.as_slice(),[Statement::If{..}])));
        let session = self.session.as_mut().ok_or_else(|| {
            self.engine
                .scene_resource_error("Startup is already complete".into())
        })?;
        let (progress, _) = self.engine.debug_step(session, kind)?;
        work=work.saturating_add(session.work.0).saturating_add(session.work.1);
        if progress == DebugProgress::Complete {
            self.prepare_session()?;
            if logical&&kind==StepKind::Continue&&!self.is_complete(){
                transitions+=1;if transitions>=32||work>=10_000{return Ok(DebugProgress::Running);}continue;
            }
            return Ok(if self.is_complete() {
                DebugProgress::Complete
            } else {
                DebugProgress::Stopped
            });
        }
        return Ok(progress)
        }
    }
    pub fn into_engine(self) -> Result<Engine<R>, RuntimeError> {
        if !self.is_complete() {
            return Err(self.engine.scene_resource_error(
                "Pending initialization cannot become a gameplay Engine".into(),
            ));
        }
        if !self.engine.frozen_startup_program.as_ref().is_some_and(|program|std::sync::Arc::ptr_eq(program,&self.program)){return Err(self.engine.scene_resource_error("Startup handoff lost its actual shared frozen source".into()));}
        Ok(self.engine)
    }
    pub fn validate_start_label(&self,label:&str)->Result<(),RuntimeError>{self.engine.resolve(label).map(|_|())}
    pub fn text_resources(&self)->&crate::string_table::StringTableResources{self.engine.text_resources()}
    pub fn locale(&self)->Option<&LocaleManager>{self.engine.locale.as_ref()}
    pub fn save15_state_for_audio_warming(&self,save:&crate::save::consumer_checkpoint::Save15)->Result<GameState,crate::save::SaveError>{self.engine.save15_state_for_audio_warming(&self.program,save)}
    pub fn capture_save15(
        &self,
        slot: u32,
        label: String,
    ) -> Result<crate::save::consumer_checkpoint::Save15, crate::save::SaveError> {
        if self.is_complete(){return Err(crate::save::SaveError::Incompatible("Completed startup must transfer its actual Engine before Save15 capture".into()));}
        self.validate_terminal_capture_phase()?;
        if self.waiting_for_video()||self.waiting_for_imagemap(){return self.capture_waiting_save15(slot,label);}
        self.engine
            .capture_startup_save15_inner(&self.program, self.session.as_ref(), slot, label,None)
    }
    /// Restore into the already admitted, uninitialized real Engine. Its Init
    /// bodies have not run, and saved progress is reconstructed from its table.
    pub fn restore_save15(
        &mut self,
        save: crate::save::consumer_checkpoint::Save15,
    ) -> Result<(), crate::save::SaveError> {
        let locale = self
            .engine
            .locale
            .as_ref()
            .map(LocaleManager::debugger_copy);
        let prepared = self.prepare_save15_with_locale(save, locale)?;
        self.begin_prepared_save15_commit(prepared)?.commit();
        Ok(())
    }
    pub fn prepare_save15_with_locale(
        &self,
        save: crate::save::consumer_checkpoint::Save15,
        locale: Option<LocaleManager>,
    ) -> Result<PreparedSave15, crate::save::SaveError> {
        use crate::save::SaveError;
        if save
            .debugger
            .as_ref()
            .and_then(|debugger| debugger.startup.as_ref())
            .is_none()
        {
            return Err(SaveError::Incompatible(
                "Startup restore requires its actual stopped Init continuation".into(),
            ));
        }
        self.engine
            .prepare_save15_with_locale(&self.program, save, locale)
    }
    pub fn begin_prepared_save15_commit(
        &mut self,
        prepared: PreparedSave15,
    ) -> Result<StartupCommitGuard<'_, R>, crate::save::SaveError> {
        use crate::save::SaveError;
        let waiting=prepared.startup.as_ref().is_some_and(|startup|startup.waiting_video.is_some()||startup.waiting_imagemap())
            &&prepared.session.is_none()&&prepared.startup_wait_configuration.is_some();
        if prepared.startup.is_none()
            || !(waiting||prepared.session.as_ref().is_some_and(|session|matches!(session.operation,DebugOperation::Startup(_))))
        {
            return Err(SaveError::Incompatible(
                "Startup commit has no admitted Init cursor/session".into(),
            ));
        }
        let prepared_videos=prepared.render_batch().videos.clone();
        let waiting_configuration=prepared.startup_wait_configuration.clone();
        let guard = self.engine.begin_prepared_save15_commit(prepared)?;
        Ok(StartupCommitGuard {
            inner: guard,
            session: &mut self.session,
            breakpoints: &mut self.breakpoints,
            watches: &mut self.watches,
            video_projection:&mut self.video_projection,
            prepared_videos,
            waiting_configuration,
        })
    }
    /// A complete-game checkpoint already contains initialization results.
    /// Consume the admitted uninitialized owner and use the existing guarded
    /// Save15 restore; never run Init to recreate the saved state or its effects.
    pub fn restore_completed_save15(
        mut self,
        save: crate::save::consumer_checkpoint::Save15,
    ) -> Result<(Engine<R>, Option<DebugSession>), crate::save::SaveError> {
        use crate::save::SaveError;
        if save
            .debugger
            .as_ref()
            .and_then(|debugger| debugger.startup.as_ref())
            .is_some()
        {
            return Err(SaveError::Incompatible(
                "A pending Init checkpoint requires its opaque startup continuation".into(),
            ));
        }
        self.engine.validate_save15_program(&self.program)?;
        save.source.validate_current(&self.program)?;
        save.encode()?;
        save.validate_enum_resources1025(&self.engine.functions.enum_assets1025)?;
        self.engine.startup = None;
        self.session = None;
        let session = self.engine.restore_save15(&self.program, save)?;
        Ok((self.engine, session))
    }
}

impl PortableStartup {
    pub(crate) fn visit_widget_value_roots_mut(&mut self, visit: &mut impl FnMut(&mut Value)->Result<(),String>)->Result<(),String>{
        for frame in &mut self.loops { for record in &mut frame.gather { record.visit_value_roots_mut(visit)?; } }
        for frame in &mut self.imagemaps {for record in &mut frame.gather{record.visit_value_roots_mut(visit)?;}}
        Ok(())
    }
}
