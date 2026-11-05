//! Transaction preparation may suspend; only a finished, exact trace reaches
//! the real renderer. All actual narrative/UI/frame wrappers remain the engine.
#[cfg(test)]
#[path="engine_debugger_tests.rs"]
mod tests;
// Reuse the real retained-batch CPU renderer for Library fixtures. Test-only;
// no production renderer capability or public runtime API is introduced.
#[cfg(test)]
pub(in crate::engine) use tests::Host as LibrarySceneTestHost1027;
#[cfg(test)]
#[path="engine_debugger_dialogue_tests_0624.rs"]
mod dialogue_binding_tests;
use super::*;
#[path="engine_debugger_macros_1020.rs"]
mod standard_macros;
#[path="engine_debugger_screen_owner.rs"]
mod screen_construction_owner;
#[path="engine_debugger_unit1003.rs"]
mod unit_live_owner;
use crate::editor_debugger::{self as wire, StepKind};
use crate::eval::debugger_eval::{Continuation, Probe, SharedProbe};
use sha2::{Digest, Sha256};
use std::fmt::Write;
#[path="engine_startup.rs"]
pub(super) mod startup;
pub use startup::{EngineStartup,StartupOperation,StartupCommitGuard,PreparedStartupStep,StartupStepCommitGuard,StartupPublicationSource,StartupBatchSink,StartupHostEffects,StartupHostStepGuard,CommittedStartupStep,StartupObservationSource,StartupVideoCommand,StartupVideoCommandKind,StartupVideoClockSource,StartupInputBatchSink,StartupImagemapSource,StartupVideoReport,PreparedStartupVideoFeedback,StartupVideoFeedbackGuard,CommittedStartupVideoFeedback,AcceptedStartupVideoReport,StartupVideoReportKind,PreparedStartupVideoContinuation,AcceptedStartupVideoTerminal,MAX_STARTUP_VIDEO_REPORTS,MAX_STARTUP_VIDEO_METADATA_BYTES};

#[derive(Clone, Debug)]
pub enum DebugOperation {
    Statement,
    Advance(crate::editor_preview::InteractionSource),
    Choice { source:crate::editor_preview::InteractionSource, index:usize },
    Hotspot { source:crate::editor_preview::InteractionSource, index:usize },
    Input(crate::ui_input::ScopedUiInput),
    MaterializeWidget(super::WidgetMaterializationOperation),
    GameFrame { seconds:f64, reports:GameFrameReports, audio:Vec<crate::audio_playback::AudioReport> },
    HostFrame { seconds:f64, reports:HostFrameReports, audio:Vec<crate::audio_playback::AudioReport> },
    RestoredFrame(super::RestoredFrameOperation),
    Startup(StartupOperation),
}
#[derive(Debug)]
pub enum DebugOutcome {
    Interaction(Option<Interaction>), Input(bool), WidgetMaterialized,
    Game((GameFrameReceiptOutcome,Vec<AudioReportReceipt>)),
    Host((HostFrameReceiptOutcome,Vec<AudioReportReceipt>)),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugProgress { Stopped, Running, Complete }

/// Opaque source for reversible renderer-base preparation. It carries no new
/// ordinal and authorizes neither payload execution nor a saved checkpoint.
pub struct DebugStateSource { instance:u64,epoch:u64,anchor:[u8;32],text_resources:crate::string_table::StringTableRevision,locale_revision:Option<crate::locale::LocaleRevision> }

/// Owned, single-use restore prepared against one unchanged live Engine.
/// Host resource authority must be revalidated by its actual owner immediately
/// before commit; no serialized/client object can manufacture this plan.
pub struct PreparedSave15 {
    source: DebugStateSource,
    widget_retirement:Option<std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>>,
    batch: crate::renderer::SceneRenderBatch,
    locale: Option<LocaleManager>,
    session: Option<DebugSession>,
    reveal_source:Option<crate::display_reveal::DisplayRevealSource>,
    choice_backdrop:Option<Interaction>,
    frame_audio:bool,
    startup:Option<startup::StartupState>,
    native_media:Option<Vec<rvn_media::checkpoint::NativeCheckpoint>>,
    startup_wait_configuration:Option<(Vec<wire::Breakpoint>,Vec<wire::Watch>)>,
}
impl PreparedSave15 {
    /// Read-only projection for the renderer owner's real resource preflight.
    pub fn render_batch(&self)->&crate::renderer::SceneRenderBatch { &self.batch }
    pub fn display_reveal_source(&self)->Option<&crate::display_reveal::DisplayRevealSource>{self.reveal_source.as_ref()}
    pub fn choice_backdrop(&self)->Option<&Interaction>{self.choice_backdrop.as_ref()}
    pub fn requires_frame_audio_checkpoint(&self)->bool{self.frame_audio}
    pub fn native_media_checkpoint(&self)->Option<&[rvn_media::checkpoint::NativeCheckpoint]>{self.native_media.as_deref()}
}

/// The existing restore preflights have succeeded. An exclusive Engine borrow
/// spans the real host fence; no caller can construct/serialize/reuse this guard.
/// Host-owned resource and PCM fences must remain valid until commit returns.
pub struct Save15CommitGuard<'a,R:Renderer>{engine:&'a mut Engine<R>,prepared:PreparedSave15,widget_reservation:Option<Box<dyn crate::widget_backend_0956::WidgetViewportCommitReservation>>}
impl<R:Renderer> Save15CommitGuard<'_,R>{
    pub fn render_batch(&self)->&crate::renderer::SceneRenderBatch{&self.prepared.batch}
    pub fn commit(self)->Option<DebugSession>{
        let Self{engine,prepared,widget_reservation}=self;
        if let Some(reservation)=widget_reservation{
            let mut prepared=Some(prepared);let mut result=None;
            reservation.commit(&mut||{result=Some(engine.publish_prepared_save15(prepared.take().expect("owned target published once")));});
            result.expect("actual parent reservation published its sole target")
        }else{engine.publish_prepared_save15(prepared)}
    }
    /// Actual Sources supply this while their producer fence is held. Validate
    /// all snapshot/anchor data before changing Engine; never reenter renderer
    /// or its broker while the host holds broker -> gate -> cursor locks.
    pub fn commit_with_audio_checkpoint(mut self,snapshot:crate::audio_checkpoint::AudioCheckpointSnapshot)->Result<Option<DebugSession>,crate::save::SaveError>{
        use crate::save::SaveError;
        let next=if self.prepared.frame_audio {
            crate::frame_audio_checkpoint::checkpointed_frame_state(&self.prepared.batch.state,self.engine.instance_id(),self.prepared.batch.runtime_epoch,&snapshot)
        }else{crate::audio_checkpoint::checkpointed_state(&self.prepared.batch.state,self.engine.instance_id(),self.prepared.batch.runtime_epoch,&snapshot)}.map_err(SaveError::Incompatible)?;
        let mut candidate=self.engine.debugger_candidate(Probe::shared());candidate.functions.debug_probe=None;if candidate.state.game_tasks.is_some(){candidate.active_timer=None;}candidate.state=next.clone();candidate.locale=self.prepared.locale.as_ref().map(LocaleManager::debugger_copy);candidate.startup=self.prepared.startup.clone();candidate.history.clear();candidate.interface_view_cache.lock().map_err(|_|SaveError::Incompatible("Restore anchor cache unavailable".into()))?.clear();
        if candidate.debugger_anchor_measured().map_err(|e|SaveError::Incompatible(e.to_string()))?.1>16*1024*1024-4096{return Err(SaveError::Incompatible("Fenced Save15 anchor lacks bounded generation headroom".into()));}
        self.prepared.batch.state=next;
        Ok(self.commit())
    }
}


#[derive(Clone, Copy, Debug)]
struct ArmedStep { kind:StepKind, depth:usize }
#[derive(Clone, Debug, PartialEq, Eq)]
struct StoppedVisit { entry:bool, site:Option<wire::Site> }

/// A session belongs to the same real Engine instance/epoch/entire source
/// state, history counter and program. It is not a second Engine authority.
pub struct DebugSession {
    instance:u64, epoch:u64, anchor:[u8;32], operation:DebugOperation,
    text_resources:crate::string_table::StringTableRevision,
    locale_revision:Option<crate::locale::LocaleRevision>,
    probe:SharedProbe, continuation:Option<Continuation>,
    position:crate::scene::ResumePosition, breakpoints:Vec<wire::Breakpoint>,
    watches:Vec<wire::Watch>, stopped_visit:Option<StoppedVisit>, armed_step:Option<ArmedStep>,
    error:Option<String>, complete:bool, entry_stop:bool,
    owns_materialization_barrier:bool,
    last_site:Option<wire::Site>,last_origin:Option<crate::program::InstructionOrigin>,work:(usize,usize),
}
impl DebugSession {
    fn startup_step_anchor(&self)->Result<[u8;32],String>{
        let probe=self.probe.lock().map_err(|_|"Debugger computation lock unavailable")?;
        let mut hash=Fingerprint{hash:Sha256::new(),bytes:0};
        write!(&mut hash,"{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
            (self.instance,self.epoch,self.anchor),self.operation,*probe,self.continuation,
            self.position,self.breakpoints,self.watches,self.stopped_visit,self.armed_step,
            (&self.error,self.complete,self.entry_stop,self.owns_materialization_barrier),(&self.last_site,&self.last_origin),self.work)
            .map_err(|_|"Startup session exceeds bounded observation".to_owned())?;
        Ok(hash.hash.finalize().into())
    }
    fn fork_startup(&self)->Result<Self,String>{
        if !matches!(self.operation,DebugOperation::Startup(_))||self.complete||self.error.is_some(){return Err("Only a current pending startup session can be forked".into());}
        self.startup_step_anchor()?;
        let probe=Probe::fork(&self.probe).map_err(|e|e.to_string())?;
        let continuation=self.continuation.as_ref().map(Continuation::fork).transpose().map_err(|e|e.to_string())?;
        Ok(Self{instance:self.instance,epoch:self.epoch,anchor:self.anchor,operation:self.operation.clone(),
            text_resources:self.text_resources.clone(),locale_revision:self.locale_revision.clone(),
            probe,continuation,position:self.position.clone(),breakpoints:self.breakpoints.clone(),watches:self.watches.clone(),
            stopped_visit:self.stopped_visit.clone(),armed_step:self.armed_step,error:None,complete:false,entry_stop:self.entry_stop,owns_materialization_barrier:self.owns_materialization_barrier,
            last_site:self.last_site.clone(),last_origin:self.last_origin.clone(),work:self.work})
    }
    pub fn snapshot(&self)->wire::Snapshot {
        if let Some(continuation)=&self.continuation {
            let mut snapshot=continuation.snapshot(self.instance,self.epoch,self.position.clone(),&self.watches);
            if self.entry_stop {if let DebugOperation::Startup(operation)=&self.operation{snapshot.site=Some(operation.entry_site());snapshot.origin=Some(operation.origin.clone());}}
            snapshot.breakpoints=self.breakpoints.clone();snapshot.armed=self.armed_step_kind();
            snapshot.running=self.armed_step.is_some()&&!self.complete&&self.error.is_none();
            if self.error.is_some(){
                snapshot.error=self.error.clone();
                if snapshot.site.is_none(){snapshot.site=self.last_site.clone();snapshot.origin=self.last_origin.clone().or(snapshot.origin);}
            }return snapshot;
        }
        let entry=if self.entry_stop{if let DebugOperation::Startup(operation)=&self.operation{Some((operation.entry_site(),operation.origin.clone()))}else{None}}else{None};
        wire::Snapshot{instance:self.instance,epoch:self.epoch,position:self.position.clone(),site:entry.as_ref().map(|(site,_)|site.clone()).or_else(||self.last_site.clone()),origin:entry.map(|(_,origin)|origin).or_else(||self.last_origin.clone()).or_else(||self.position.origin.clone()),
            frames:Vec::new(),watches:self.watches.iter().map(|watch|wire::ObservedWatch{watch:watch.clone(),value:wire::WatchValue::NoDebugObject}).collect(),
            evaluating:!self.complete,running:false,armed:self.armed_step_kind(),candidate:false,steps:self.work.0,value_work:self.work.1,error:self.error.clone(),sites:None,breakpoints:self.breakpoints.clone()}
    }
    pub fn set_watches(&mut self,watches:Vec<wire::Watch>)->Result<(),String> {
        if watches.len()>wire::MAX_WATCHES||watches.iter().any(|watch|!rvn_parser::is_variable_binding_name(&watch.name)||watch.name.len()>4096){return Err("Invalid or oversized debugger watches".into());}
        self.watches=watches;Ok(())
    }
    pub fn is_complete(&self)->bool {self.complete}
    pub fn operation(&self)->&DebugOperation{&self.operation}
    pub fn configuration(&self)->(&[wire::Breakpoint],&[wire::Watch]){(&self.breakpoints,&self.watches)}
    pub fn armed_step_kind(&self)->Option<StepKind>{self.armed_step.map(|step|step.kind)}
    fn visit(&self)->StoppedVisit {
        StoppedVisit{entry:self.entry_stop,site:if self.entry_stop{if let DebugOperation::Startup(operation)=&self.operation{Some(operation.entry_site())}else{self.continuation.as_ref().and_then(Continuation::site)}}else{self.continuation.as_ref().and_then(Continuation::site)}}
    }
    fn stopped(&mut self) {
        self.stopped_visit=Some(self.visit());self.armed_step=None;
    }
    fn breakpoint(&self)->bool {
        let site=self.visit().site;
        self.breakpoints.iter().any(|breakpoint|breakpoint.enabled
            &&breakpoint.site.as_ref().map_or(self.entry_stop&&breakpoint.position==self.position,|expected|site.as_ref().is_some_and(|actual|
                (expected.function.is_some()||breakpoint.position==self.position)&&
                actual.function==expected.function&&actual.route==expected.route&&actual.statement==expected.statement)))
    }
}

struct Fingerprint { hash:Sha256, bytes:usize }
impl std::fmt::Write for Fingerprint {
    fn write_str(&mut self,value:&str)->std::fmt::Result {
        self.bytes=self.bytes.checked_add(value.len()).ok_or(std::fmt::Error)?;
        if self.bytes>16*1024*1024{return Err(std::fmt::Error);}
        self.hash.update(value.as_bytes());Ok(())
    }
}
impl<R:Renderer> Engine<R> {
    /// Opt-in startup for the authenticated editor player. Ordinary game
    /// construction performs no sidecar preparation or additional disk reads.
    pub fn from_program_with_debugger(program:rvn_parser::ParsedProgram,renderer:R,rollback_depth:usize)->Result<Self,RuntimeError>{
        Self::from_program_with_debugger_and_persistent(program,renderer,rollback_depth,HashMap::new())
    }
    pub fn from_program_with_debugger_and_persistent(program:rvn_parser::ParsedProgram,renderer:R,rollback_depth:usize,persistent_vars:HashMap<String,Value>)->Result<Self,RuntimeError>{
        Self::from_program_with_debugger_and_text_context(program,renderer,rollback_depth,persistent_vars,Default::default())
    }
    pub fn from_program_with_debugger_and_text_context(program:rvn_parser::ParsedProgram,renderer:R,rollback_depth:usize,persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext)->Result<Self,RuntimeError>{
        let map=crate::debugger_provenance::SourceMap::from_program(&program).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(error)),0))?;
        let binding=crate::consumer_schedule::ConsumerSourceBinding::from_program(&program).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        let mut engine=Self::from_program_bootstrap_with_consumer_binding(program,renderer,rollback_depth,persistent_vars,text_context,false,Some(binding))?;
        engine.functions.debug_origins=map.sites.clone();engine.functions.debug_source_map=Some(map);Ok(engine)
    }
    pub fn debug_current_snapshot(&self,watches:&[wire::Watch])->Result<wire::Snapshot,String>{
        if watches.len()>wire::MAX_WATCHES||watches.iter().any(|watch|!rvn_parser::is_variable_binding_name(&watch.name)||watch.name.len()>4096){return Err("Invalid debugger watches".into());}
        let position=self.narrative.as_ref().map(|program|program.position(self.state.pc)).unwrap_or(crate::scene::ResumePosition{pc:self.state.pc,origin:None});
        let mut remaining=wire::MAX_SNAPSHOT_BYTES;
        let observed=watches.iter().map(|watch|{
            remaining=remaining.saturating_sub(watch.name.len()+64);
            let value=match watch.scope{
                wire::WatchScope::Globals=>self.state.vars.get(&watch.name).map(|value|wire::bounded_value_with_budget(value,&mut remaining)).unwrap_or(wire::WatchValue::NoProperty),
                wire::WatchScope::Frame(_)=>wire::WatchValue::NoDebugObject,
            };wire::ObservedWatch{watch:watch.clone(),value}
        }).collect();
        let origin=self.functions.debug_source_map.as_ref().and_then(|map|map.origins.get(self.state.pc)).cloned().or_else(||position.origin.clone());
        Ok(wire::Snapshot{instance:self.instance_id(),epoch:self.runtime_epoch(),origin,position,
            site:None,frames:Vec::new(),watches:observed,evaluating:false,running:false,armed:None,candidate:false,steps:0,value_work:0,error:None,sites:None,breakpoints:Vec::new()})
    }
    /// Full eligible catalogue, paged without cloning all program sites. Each
    /// offered entry is validated by the same actual AST/IR admission as a BP.
    pub fn debug_site_page(&self,offset:usize)->Result<wire::SitePage,RuntimeError>{
        self.validate_live_unit_debug(None)?;
        let mut count=Fingerprint{hash:Sha256::new(),bytes:0};
        write!(&mut count,"{:?}{:?}",self.script,self.functions.debug_origins).map_err(|_|self.scene_resource_error("Debugger site catalogue exceeds its metadata budget".into()))?;
        let declarations:std::collections::HashMap<_,_>=self.script.iter().enumerate().flat_map(|(pc,statement)|wire::declaration_scope_keys(statement).into_iter().map(move|scope|(scope,pc))).collect();
        let position=|pc|self.narrative.as_ref().map(|program|program.position(pc)).unwrap_or(crate::scene::ResumePosition{pc,origin:None});
        let mut total=0usize;let mut entries=Vec::new();
        for(pc,statement)in self.script.iter().enumerate(){
            if wire::declaration_scope(statement).is_some(){continue;}
            let breakpoint=wire::Breakpoint{position:position(pc),site:None,enabled:true};
            if self.validate_debug_breakpoints(std::slice::from_ref(&breakpoint)).is_err(){continue;}
            if total>=offset&&entries.len()<64{let origin=self.functions.debug_source_map.as_ref().and_then(|map|map.origins.get(pc)).cloned().or_else(||breakpoint.position.origin.clone());entries.push(wire::SiteEntry{origin,breakpoint});}total+=1;
        }
        for source in &self.functions.debug_origins{
            let pc=if source.scope.is_empty(){
                let Some(pc)=self.functions.debug_source_map.as_ref().and_then(|map|map.origins.iter().enumerate().find_map(|(pc,origin)|
                    (origin.source==source.declaration.source&&origin.span==source.declaration.span&&(matches!(self.script.get(pc),Some(Statement::ScheduledConsumer{..}))||self.startup.is_some()&&matches!(self.script.get(pc),Some(Statement::Init{..})))).then_some(pc)))else{continue;};pc
            }else{let Some(&pc)=declarations.get(&source.scope)else{continue;};pc};
            let site=wire::Site{function:(!source.scope.is_empty()).then(||source.scope.clone()),route:source.route.clone(),depth:0,statement:source.origin.scheduled_step.is_none()};
            if !wire::validate_site(&self.script[pc],&site){continue;}
            if total>=offset&&entries.len()<64{entries.push(wire::SiteEntry{breakpoint:wire::Breakpoint{position:position(pc),site:Some(site),enabled:true},origin:Some(source.origin.clone())});}total+=1;
        }
        if offset>total{return Err(self.scene_resource_error("Debugger site page is outside the actual programme".into()));}
        Ok(wire::SitePage{offset,total,entries})
    }
    pub fn debug_state_source(&self)->Result<DebugStateSource,RuntimeError>{
        Ok(DebugStateSource{instance:self.instance_id(),epoch:self.runtime_epoch(),anchor:self.debugger_anchor()?,text_resources:self.functions.text_resources().revision_stamp(),locale_revision:self.locale.as_ref().map(LocaleManager::revision_stamp)})
    }
    pub fn accepts_debug_state_source(&self,source:&DebugStateSource)->bool{
        source.instance==self.instance_id()&&source.epoch==self.runtime_epoch()
            &&source.text_resources==self.functions.text_resources().revision_stamp()
            &&source.locale_revision==self.locale.as_ref().map(LocaleManager::revision_stamp)
            &&self.debugger_anchor().is_ok_and(|anchor|anchor==source.anchor)
    }
    fn debugger_anchor(&self)->Result<[u8;32],RuntimeError> {Ok(self.debugger_anchor_measured()?.0)}
    pub(super) fn debugger_anchor_measured(&self)->Result<([u8;32],usize),RuntimeError> {
        let mut hash=Fingerprint{hash:Sha256::new(),bytes:0};
        write!(&mut hash,"{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",self.state,self.script,self.input_state,self.active_timer,
            self.persistent_vars,self.host_random,(self.interface_epoch,self.video_epoch,self.runtime_epoch),
            (self.display_generation,self.display_owner,self.display_history_entry,&self.legacy_preview_visit),self.engine_instance_id,(&self.startup,self.restored_widget_barrier))
            .map_err(|_|self.scene_resource_error("Debugger source exceeds the bounded observation".into()))?;
        self.history.debugger_write(&mut hash).map_err(|_|self.scene_resource_error("Debugger history exceeds the bounded observation".into()))?;
        if let Some(locale)=&self.locale{locale.debugger_write(&mut hash).map_err(|_|self.scene_resource_error("Debugger locale exceeds the bounded observation".into()))?;}
        let cache=self.interface_view_cache.lock().map_err(|_|self.scene_resource_error("Debugger interface cache unavailable".into()))?;
        for entry in cache.iter(){
            write!(&mut hash,"{:?}{:?}{:?}",entry.ui,entry.globals,entry.views).map_err(|_|self.scene_resource_error("Debugger interface cache exceeds the bounded observation".into()))?;
            if let Some(locale)=&entry.locale{locale.debugger_write(&mut hash).map_err(|_|self.scene_resource_error("Debugger cached locale exceeds the bounded observation".into()))?;}
        }
        Ok((hash.hash.finalize().into(),hash.bytes))
    }
    pub fn validate_debug_breakpoints(&self,breakpoints:&[wire::Breakpoint])->Result<(),RuntimeError> {
        if breakpoints.len()>wire::MAX_BREAKPOINTS{return Err(self.scene_resource_error("Too many debugger breakpoints".into()));}
        for breakpoint in breakpoints {
            if let Some(program)=&self.narrative{program.validate_position(&breakpoint.position).map_err(|message|self.scene_resource_error(message))?;}
            else if breakpoint.position.pc>=self.script.len()||breakpoint.position.origin.is_some(){return Err(self.scene_resource_error("Breakpoint has no emitted instruction".into()));}
            let root=self.script.get(breakpoint.position.pc).ok_or_else(||self.scene_resource_error("Breakpoint has no emitted instruction".into()))?;
            if let Some(site)=&breakpoint.site{
                if matches!(root,Statement::Init{..})&&!self.startup.as_ref().is_some_and(|startup|startup.roots.iter().any(|root|root.pc==breakpoint.position.pc)) {return Err(self.scene_resource_error("Initializer breakpoint requires its pending actual startup owner".into()));}
                let producer=if site.statement{true}else{
                    self.functions.debug_origins.iter().any(|record|record.origin.scheduled_step.is_some()&&record.route==site.route&&
                        if let Some(scope)=&site.function{record.scope==*scope}else{record.scope.is_empty()&&self.functions.debug_source_map.as_ref().and_then(|map|map.origins.get(breakpoint.position.pc)).is_some_and(|actual|actual.source==record.declaration.source&&actual.span==record.declaration.span)})
                };
                if !producer||!wire::validate_site(root,site){return Err(self.scene_resource_error("Breakpoint has no actual emitted statement site".into()));}
            }else if wire::declaration_scope(root).is_some()||matches!(root,Statement::Use{..}|Statement::Init{..}){
                return Err(self.scene_resource_error("Declaration has no independent runtime breakpoint".into()));
            }
        }
        Ok(())
    }
    /// Validate the complete replacement before changing a suspended command.
    pub fn configure_debug_session(&self,session:&mut DebugSession,breakpoints:Vec<wire::Breakpoint>,watches:Vec<wire::Watch>)->Result<(),RuntimeError>{
        if !self.accepts_debug_session(session){return Err(self.scene_resource_error("Debugger configuration belongs to a stale source".into()));}
        self.validate_debug_breakpoints(&breakpoints)?;
        if watches.len()>wire::MAX_WATCHES||watches.iter().any(|watch|!rvn_parser::is_variable_binding_name(&watch.name)||watch.name.len()>4096){return Err(self.scene_resource_error("Invalid or oversized debugger watches".into()));}
        session.breakpoints=breakpoints;session.watches=watches;Ok(())
    }
    pub fn debug_begin(&self,operation:DebugOperation,breakpoints:Vec<wire::Breakpoint>)->Result<DebugSession,RuntimeError> {
        if self.restored_widget_barrier{return Err(self.scene_resource_error("The actual saved operation exclusively owns the Widget materialization boundary".into()));}
        self.debug_begin_owned(operation,breakpoints,false)
    }
    /// Only the Save15 hydration candidate calls this after restore_host_input
    /// authenticated its exact current target, frozen handler and Screen roots.
    /// A resaved stopped operation already has real pending materialization;
    /// that logical continuation is not a new external presentation grant.
    fn debug_begin_restored_host_input(&mut self,input:crate::ui_input::ScopedUiInput,
        breakpoints:Vec<wire::Breakpoint>)->Result<DebugSession,RuntimeError>{
        if self.restored_widget_barrier||!self.has_pending_widget_materialization(){return Err(self.scene_resource_error("Restored Host input requires its actual pending materialization candidate".into()));}
        self.portable_host_input(&input).map_err(|message|self.scene_resource_error(message))?;
        self.restored_widget_barrier=true;
        self.debug_begin_owned(DebugOperation::Input(input),breakpoints,true)
    }
    fn debug_begin_owned(&self,operation:DebugOperation,breakpoints:Vec<wire::Breakpoint>,owns_materialization_barrier:bool)->Result<DebugSession,RuntimeError>{
        self.validate_live_unit_debug(Some(&operation))?;
        self.validate_debug_breakpoints(&breakpoints)?;
        let position=self.state.scene_execution.as_ref().map(|execution|execution.live_position.clone()).unwrap_or(crate::scene::ResumePosition{pc:self.state.pc,origin:None});
        let mut session=DebugSession{instance:self.instance_id(),epoch:self.runtime_epoch(),anchor:self.debugger_anchor()?,operation,
            text_resources:self.functions.text_resources().revision_stamp(),locale_revision:self.locale.as_ref().map(LocaleManager::revision_stamp),
            probe:Probe::shared(),continuation:None,position,breakpoints,watches:Vec::new(),stopped_visit:None,armed_step:None,error:None,complete:false,entry_stop:true,owns_materialization_barrier,last_site:None,last_origin:None,work:(0,0)};
        self.debug_prepare(&mut session)?;Ok(session)
    }
    pub fn accepts_debug_session(&self,session:&DebugSession)->bool {
        (!self.restored_widget_barrier||session.owns_materialization_barrier)&&session.instance==self.instance_id()&&session.epoch==self.runtime_epoch()&&session.text_resources==self.functions.text_resources().revision_stamp()&&session.locale_revision==self.locale.as_ref().map(LocaleManager::revision_stamp)&&self.debugger_anchor().is_ok_and(|anchor|anchor==session.anchor)
    }
    pub fn debug_session_snapshot(&self,session:&DebugSession)->wire::Snapshot {
        let mut snapshot=session.snapshot();
        if !session.complete{return snapshot;}
        // A completed callee has no live lexical object. Globals come from the
        // real committed state, rather than a candidate that preceded Set/UI.
        let mut remaining=wire::MAX_SNAPSHOT_BYTES;
        snapshot.watches=session.watches.iter().map(|watch|{
            remaining=remaining.saturating_sub(watch.name.len()+64);
            let value=match watch.scope {
                wire::WatchScope::Globals=>self.state.vars.get(&watch.name).map(|value|wire::bounded_value_with_budget(value,&mut remaining)).unwrap_or(wire::WatchValue::NoProperty),
                wire::WatchScope::Frame(_)=>wire::WatchValue::NoDebugObject,
            };wire::ObservedWatch{watch:watch.clone(),value}
        }).collect();snapshot.frames.clear();snapshot
    }
    pub fn abandon_restored_widget_boundary(&mut self,mut session:DebugSession)->Result<(),RuntimeError>{
        if !self.accepts_debug_session(&session)||session.complete{return Err(self.scene_resource_error("Abandoned saved operation is not the actual current owner".into()));}
        if session.owns_materialization_barrier{self.restored_widget_barrier=false;session.owns_materialization_barrier=false;}
        Probe::revoke_screen_construction(&session.probe);Ok(())
    }
    fn widget_materialization_entry(&self,session:&DebugSession)->Result<bool,crate::save::SaveError>{
        if !self.accepts_debug_session(session){return Err(crate::save::SaveError::Incompatible("Widget entry no longer belongs to its actual source".into()));}
        Ok(matches!(session.operation,DebugOperation::MaterializeWidget(_))&&session.entry_stop&&session.stopped_visit.is_none()&&session.last_site.is_none()&&session.work==(0,0)
            &&session.continuation.is_none()&&!session.complete&&session.error.is_none()&&Probe::complete(&session.probe))
    }
    fn debug_prepare(&self,session:&mut DebugSession)->Result<(),RuntimeError> {
        if !self.accepts_debug_session(session){Probe::revoke_screen_construction(&session.probe);return Err(self.scene_resource_error("Debugger source changed before preparation".into()));}
        Probe::rewind(&session.probe).map_err(|error|self.eval_err(error,"Debugger"))?;
        Probe::begin_screen_preparation(&session.probe,self,session).map_err(|error|self.eval_err(error,"Screen construction"))?;
        let mut candidate=self.debugger_candidate(session.probe.clone());
        // This evaluator discovery is discarded. Only the later accepted real
        // startup operation may issue host video-action observations.
        candidate.startup_video_trace=None;
        let result=candidate.debug_operation(session.operation.clone(),session.owns_materialization_barrier);
        let pending=Probe::pending(&session.probe,&self.functions).map_err(|error|self.eval_err(error,"Debugger"))?;
        if let Some(pending)=pending{session.continuation=Some(pending);return Ok(());}
        match result {
            Ok(_) if Probe::complete(&session.probe)=>{session.continuation=None;Ok(())},
            Ok(_)=>Err(self.scene_resource_error("Debugger computation trace did not finish".into())),
            Err(error)=>{Probe::revoke_screen_construction(&session.probe);session.error=Some(error.to_string());Err(error)},
        }
    }
    /// One drain is bounded. Continue/Over/Out retain their continuation and
    /// budget across drains rather than creating a second playhead or clock.
    pub fn debug_step(&mut self,session:&mut DebugSession,kind:StepKind)->Result<(DebugProgress,Option<DebugOutcome>),RuntimeError> {
        if session.complete{return Err(self.scene_resource_error("Debugger command already completed".into()));}
        if session.error.is_some(){return Err(self.scene_resource_error("The failed debugger computation cannot be resumed".into()));}
        if !self.accepts_debug_session(session){Probe::revoke_screen_construction(&session.probe);return Err(self.scene_resource_error("Debugger command belongs to a stale Engine source".into()));}
        if session.armed_step.is_some_and(|armed|armed.kind!=kind){return Err(self.scene_resource_error("A debugger step is still running; resume that command before replacing it".into()));}
        if kind==StepKind::Out&&session.armed_step.is_none()&&session.continuation.as_ref().map_or(0,Continuation::depth)==0{return Err(self.scene_resource_error("No caller frame to step out to".into()));}
        let result=(||{if let DebugOperation::MaterializeWidget(operation)=&mut session.operation{self.bind_widget_materialization(operation)?;}self.debug_step_inner(session,kind)})();
        if let Err(error)=&result{Probe::revoke_screen_construction(&session.probe);session.error=Some(error.to_string());session.armed_step=None;}
        result
    }
    fn debug_step_inner(&mut self,session:&mut DebugSession,kind:StepKind)->Result<(DebugProgress,Option<DebugOutcome>),RuntimeError> {
        self.debug_step_inner_budget(session,kind,4096,4096)
    }
    fn debug_step_inner_budget(&mut self,session:&mut DebugSession,kind:StepKind,budget:usize,administrative_budget:usize)->Result<(DebugProgress,Option<DebugOutcome>),RuntimeError> {
        let start_depth=if let Some(armed)=session.armed_step {
            if armed.kind!=kind{return Err(self.scene_resource_error("A debugger step is still running; resume that command before replacing it".into()));}
            armed.depth
        }else{
            let depth=session.continuation.as_ref().map_or(0,Continuation::depth);
            if kind==StepKind::Out&&depth==0{return Err(self.scene_resource_error("No caller frame to step out to".into()));}
            session.armed_step=Some(ArmedStep{kind,depth});depth
        };
        let mut advanced=false;
        for _ in 0..budget {
            if session.breakpoint()&&session.stopped_visit.as_ref()!=Some(&session.visit())&&(!session.entry_stop||kind==StepKind::Continue) {
                session.stopped();return Ok((DebugProgress::Stopped,None));
            }
            // A stopped visit is skipped exactly once, whatever stopped it.
            // Executing this visit revokes the latch; a later loop/recursive
            // visit to the same authored route is eligible again.
            session.stopped_visit=None;
            session.entry_stop=false;
            if let Some(continuation)=&mut session.continuation {
                if let Some(site)=continuation.site(){session.last_origin=continuation.site_origin(&site,&session.position);session.last_site=Some(site);}
                let result=continuation.step_with_budget(administrative_budget);advanced=true;
                session.work=continuation.work();
                if continuation.done(){Probe::finish(&session.probe,continuation).map_err(|error|self.eval_err(error,"Debugger"))?;self.debug_prepare(session)?;}
                result.map_err(|error|self.eval_err(error,"Debugger evaluator"))?;
            }else{
                // Preparation succeeded. The real operation sees exactly the
                // completed computations and publishes through existing wrappers.
                if !self.accepts_debug_session(session){return Err(self.scene_resource_error("Debugger source changed before publication".into()));}
                Probe::seal(&session.probe).map_err(|error|self.eval_err(error,"Debugger"))?;
                Probe::rewind(&session.probe).map_err(|error|self.eval_err(error,"Debugger"))?;
                let mut library=self.functions.clone();library.debug_probe=Some(session.probe.clone());
                let original=std::mem::replace(&mut self.functions,library);
                let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||self.debug_operation(session.operation.clone(),session.owns_materialization_barrier)));
                self.functions=original;
                let result=match result{Ok(result)=>result,Err(panic)=>std::panic::resume_unwind(panic)};
                let outcome=result?;
                if !Probe::complete(&session.probe){return Err(self.scene_resource_error("Published debugger computation trace changed".into()));}
                Probe::revoke_screen_construction(&session.probe);if session.owns_materialization_barrier{self.restored_widget_barrier=false;session.owns_materialization_barrier=false;}session.complete=true;session.armed_step=None;session.stopped_visit=None;return Ok((DebugProgress::Complete,Some(outcome)));
            }
            let depth=session.continuation.as_ref().map_or(0,Continuation::depth);
            if advanced&&match kind{StepKind::Into=>session.continuation.as_ref().is_some_and(|continuation|continuation.site().is_some()),StepKind::Over=>depth<=start_depth&&session.continuation.as_ref().is_some_and(|continuation|continuation.site().is_some()),StepKind::Out=>depth<start_depth&&session.continuation.as_ref().is_some_and(|continuation|continuation.site().is_some()),StepKind::Continue=>false}{session.stopped();return Ok((DebugProgress::Stopped,None));}
        }
        Ok((DebugProgress::Running,None))
    }
    fn debug_operation(&mut self,operation:DebugOperation,restored_input_boundary:bool)->Result<DebugOutcome,RuntimeError> {
        match operation {
            DebugOperation::Statement=>self.preview_step_statement().map(DebugOutcome::Interaction),
            DebugOperation::Advance(source)=>{if !self.accepts_preview_input_source(&source){return Err(self.scene_resource_error("Captured debug dialogue changed".into()));}self.advance_dialogue()?;Ok(DebugOutcome::Interaction(None))},
            DebugOperation::Choice{source,index}=>{if !self.accepts_preview_input_source(&source)||!source.choice_indices.contains(&index){return Err(self.scene_resource_error("Captured debug choice changed".into()));}let selected=if self.script.get(self.state.pc).and_then(Self::consumer_choice_options).is_some(){source.choice_indices.iter().position(|actual|*actual==index).ok_or_else(||self.scene_resource_error("Choice input lost its actual visible row".into()))?}else{index};self.submit_choice(selected)?;Ok(DebugOutcome::Interaction(None))},
            DebugOperation::Hotspot{source,index}=>{if !self.accepts_preview_input_source(&source)||!source.choice_indices.contains(&index){return Err(self.scene_resource_error("Captured debug hotspot changed".into()));}self.submit_hotspot(index)?;Ok(DebugOutcome::Interaction(None))},
            DebugOperation::Input(input)=>if restored_input_boundary{self.resume_restored_host_input(input).map(DebugOutcome::Input)}else{self.interface_event_scoped(input).map(DebugOutcome::Input)},
            DebugOperation::MaterializeWidget(operation)=>self.execute_widget_materialization(operation).map(|_|DebugOutcome::WidgetMaterialized),
            DebugOperation::GameFrame{seconds,reports,audio}=>self.tick_game_frame_with_receipts_and_audio(seconds,reports,audio,true).map(DebugOutcome::Game),
            DebugOperation::HostFrame{seconds,reports,audio}=>self.tick_host_frame_with_audio(seconds,reports,audio,true).map(DebugOutcome::Host),
            DebugOperation::RestoredFrame(operation)=>operation.execute(self),
            DebugOperation::Startup(operation)=>self.execute_startup_statement(operation).map(|_|DebugOutcome::Interaction(None)),
        }
    }
    pub(super) fn debugger_candidate(&self,probe:SharedProbe)->Engine<PlanningRenderer<'_,R>> {
        let mut functions=self.functions.clone();functions.debug_probe=Some(probe);
        let cache=self.interface_view_cache.lock().unwrap_or_else(|poison|poison.into_inner()).iter().map(|entry|InterfaceViewSnapshot{locale:entry.locale.as_ref().map(LocaleManager::debugger_copy),text_resources:entry.text_resources.clone(),ui:entry.ui.clone(),globals:entry.globals.clone(),views:entry.views.clone()}).collect();
        Engine{script:self.script.clone(),functions,consumer_program:self.consumer_program.clone(),widget_classes:self.widget_classes.clone(),pending_widget_backend:None,planning_load_retirement:self.planning_load_retirement.clone(),retirement_speech:self.retirement_speech.clone(),restored_widget_barrier:self.restored_widget_barrier,consumer_temporary_writes:self.consumer_temporary_writes.clone(),initializing_consumers:self.initializing_consumers,consumer_init_pc:self.consumer_init_pc,startup:self.startup.clone(),frozen_startup_program:self.frozen_startup_program.clone(),startup_video_trace:self.startup_video_trace.clone(),ui_library:self.ui_library.clone(),interface_view_cache:Mutex::new(cache),
            #[cfg(test)]interface_view_builds:std::sync::atomic::AtomicUsize::new(0),
            label_table:self.label_table.clone(),branch_returns:self.branch_returns.clone(),state:self.state.clone(),renderer:PlanningRenderer(&self.renderer,None,None),
            history:self.history.debugger_copy(),locale:self.locale.as_ref().map(LocaleManager::debugger_copy),persistent_vars:self.persistent_vars.clone(),input_state:self.input_state.clone(),active_timer:self.active_timer.clone(),
            video_epoch:self.video_epoch,interface_epoch:self.interface_epoch,engine_instance_id:self.engine_instance_id,display_generation:self.display_generation,
            display_owner:self.display_owner,display_history_entry:self.display_history_entry,legacy_preview_visit:self.legacy_preview_visit.clone(),init_scan_end:self.init_scan_end,narrative:self.narrative.clone(),host_random:self.host_random.clone(),host_random_planning:Default::default(),runtime_epoch:self.runtime_epoch}
    }
}

/// Readonly capability/preflight delegation. Preparation may create disposable
/// renderer intents, never ECS entities, queues, file writes or hardware calls.
pub(super) struct PlanningRenderer<'a,R:Renderer>(&'a R,pub(super) Option<std::rc::Rc<std::cell::RefCell<Option<crate::renderer::SceneRenderBatch>>>>,pub(super) Option<std::rc::Rc<std::cell::RefCell<LegacyIntents>>>);
impl<R:Renderer> Renderer for PlanningRenderer<'_,R> {
    fn prepare_widget_viewport_batch(&self,owners:&[crate::widget_backend_0956::WidgetBackendOwner],transition:Option<&dyn crate::widget_backend_0956::PreparedWidgetViewport>)->Result<std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>,String>{self.0.prepare_widget_viewport_batch(owners,transition)}
    fn validate_widget_viewport_batch(&self,batch:&dyn crate::widget_backend_0956::PreparedWidgetViewportBatch)->Result<(),String>{self.0.validate_widget_viewport_batch(batch)}
    fn commit_widget_scene_batch_atomic(&mut self,batch:crate::renderer::SceneRenderBatch,widget:std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>,commit:&mut dyn FnMut(&crate::renderer::SceneRenderBatch))->Result<(),String>{self.validate_scene_batch(&batch)?;self.validate_widget_viewport_batch(widget.as_ref())?;commit(&batch);self.publish_scene_batch(batch);Ok(())}
    fn prepare_widget_viewport(&self,owner:&crate::widget_backend_0956::WidgetBackendOwner,operation:crate::widget_backend_0956::WidgetViewportOperation)->Result<std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewport>,String>{self.0.prepare_widget_viewport(owner,operation)}
    fn validate_widget_viewport(&self,prepared:&dyn crate::widget_backend_0956::PreparedWidgetViewport)->Result<(),String>{self.0.validate_widget_viewport(prepared)}
    fn publish_scene_batch(&mut self,batch:crate::renderer::SceneRenderBatch){if let Some(target)=&self.1{*target.borrow_mut()=Some(batch);}}
    fn capture_audio_checkpoint(&self,state:&GameState,instance:u64,epoch:u64)->Result<crate::audio_checkpoint::AudioCheckpointSnapshot,String>{self.0.capture_audio_checkpoint(state,instance,epoch)}
    fn capture_frame_audio_checkpoint(&self,state:&GameState,instance:u64,epoch:u64)->Result<crate::audio_checkpoint::AudioCheckpointSnapshot,String>{self.0.capture_frame_audio_checkpoint(state,instance,epoch)}
    fn validate_frame_audio_restore(&self,state:&GameState)->Result<(),String>{self.0.validate_frame_audio_restore(state)}
    fn validate_audio_restore(&self,state:&GameState)->Result<(),String>{self.0.validate_audio_restore(state)}
    fn validate_scene_batch(&self,batch:&crate::renderer::SceneRenderBatch)->Result<(),String>{self.0.validate_scene_batch(batch)}
    fn validate_menu_request(&self,effect:&rvn_ui::source_menus::MenuEffect,state:&GameState)->Result<(),String>{self.0.validate_menu_request(effect,state)}
    fn menu_request(&mut self,effect:&rvn_ui::source_menus::MenuEffect,_previous:&GameState,state:&GameState)->Result<(),String>{self.0.validate_menu_request(effect,state)}
    fn menu_request_pending(&self)->bool{self.0.menu_request_pending()}
    fn supports_accessibility(&self)->bool{self.0.supports_accessibility()}
    fn validate_accessibility_speech(&self,request:&rvn_ui::accessibility::SpeechRequest)->Result<(),String>{self.0.validate_accessibility_speech(request)}
    fn update_accessibility(&mut self,_settings:&rvn_ui::accessibility::AccessibilitySettings)->Result<(),String>{Ok(())}
    fn accessibility_speech(&mut self,request:&rvn_ui::accessibility::SpeechRequest)->Result<(),String>{self.0.validate_accessibility_speech(request)}
    fn supports_video(&self)->bool{self.0.supports_video()}
    fn supports_layered_characters(&self)->bool{self.0.supports_layered_characters()}
    fn supports_composable_motion(&self)->bool{self.0.supports_composable_motion()}
    fn supports_programmable_ui(&self)->bool{self.0.supports_programmable_ui()}
    fn supports_custom_canvas(&self)->bool{self.0.supports_custom_canvas()}
    fn update_videos(&mut self,_views:&[crate::video::VideoView])->Result<(),String>{Ok(())}
    fn update_layered_characters(&mut self,_views:&[crate::composition::LayeredView])->Result<(),String>{Ok(())}
    fn update_motions(&mut self,_views:&[crate::motion::MotionView])->Result<(),String>{Ok(())}
    fn update_interfaces(&mut self,_views:&[rvn_ui::programmable::ScreenView])->Result<(),String>{Ok(())}
    fn set_background(&mut self,path:&str,transition:&Transition){self.legacy(&(path,transition),||LegacyIntent::Background(path.into(),transition.clone()));}
    fn show_cinematic(&mut self,id:&str,transition:Option<&str>){self.legacy(&(id,transition),||LegacyIntent::Cinematic(Some(id.into()),transition.map(str::to_owned)));}
    fn hide_cinematic(&mut self,transition:Option<&str>){self.legacy(&transition,||LegacyIntent::Cinematic(None,transition.map(str::to_owned)));}
    fn unlock_ending(&mut self,id:&str){self.legacy(&id,||LegacyIntent::Ending(id.into()));}
    fn set_typewriter_config(&mut self,speed:f32){self.legacy(&speed,||LegacyIntent::Typewriter(speed));}
    fn show_sprite(&mut self,id:&str,emotion:Option<&str>,position:&Position,transition:&Transition,from:Option<&SpriteState>){self.legacy(&(id,emotion,position,transition,from),||LegacyIntent::Show(id.into(),emotion.map(str::to_owned),position.clone(),transition.clone(),from.cloned()));}
    fn hide_sprite(&mut self,id:&str,transition:&Transition,from:&SpriteState){self.legacy(&(id,transition,from),||LegacyIntent::Hide(id.into(),transition.clone(),from.clone()));}
    fn move_sprite(&mut self,id:&str,position:&Position,transition:&Transition,from:&SpriteState){self.legacy(&(id,position,transition,from),||LegacyIntent::Move(id.into(),position.clone(),transition.clone(),from.clone()));}
    fn animate_sprite(&mut self,id:&str,animation:&str,params:&[rvn_parser::AnimationParam]){self.legacy(&(id,animation,params),||LegacyIntent::Animate(id.into(),animation.into(),params.to_vec()));}
    fn stop_sprite_animation(&mut self,id:&str){self.legacy(&id,||LegacyIntent::StopAnimation(id.into()));}
    fn set_sprite_effect(&mut self,id:&str,x:Option<bool>,y:Option<bool>,scale:Option<f32>,rotation:Option<f32>,tint:Option<&str>){self.legacy(&(id,x,y,scale,rotation,tint),||LegacyIntent::Effect(id.into(),x,y,scale,rotation,tint.map(str::to_owned)));}
    fn show_dialogue(&mut self,_character:Option<&str>,_text:&str){}
    fn show_choice(&mut self,_options:&[String])->usize{0}
    fn show_imagemap(&mut self,_background:&str,_hover:Option<&str>,_hotspots:&[rvn_parser::Hotspot])->usize{0}
    fn music_play(&mut self,_file:&str,_transition:&Transition,_previous:Option<&str>){}
    fn music_stop(&mut self,_transition:&Transition){}
    fn music_set_volume(&mut self,_level:f32){}
    fn sfx_play(&mut self,_file:&str,_transition:&Transition){}
    fn sfx_stop(&mut self,_file:&str,_transition:&Transition){}
}

impl<R:Renderer> Engine<R>{
 pub fn install_debug_source_map(&mut self,overlay:crate::debugger_provenance::Overlay,nonce:u64)->Result<(),String>{
  let actual=self.functions.debug_source_map.as_ref().ok_or("Engine has no frozen parser provenance")?;
  let map=overlay.into_verified_map(actual,nonce)?;
  if self.functions.debug_source_map_installed{return Err("Debugger compiler source map was already installed".into());}
  // Only disposable observation metadata changes; runtime state and private
  // function storage retain their existing ownership and visibility.
  self.functions.debug_origins=map.sites.clone();
  self.functions.debug_source_map=Some(map);self.functions.debug_source_map_installed=true;Ok(())
 }
}

impl<R:Renderer> Engine<R> {
    pub(super) fn validate_save15_program(&self,program:&rvn_parser::ParsedProgram)->Result<(),crate::save::SaveError>{
        self.functions.reject_unit_portable_owner(&self.script).map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;
        FunctionLibrary::reject_unadmitted_unit_owner(&program.script).map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;
        if !self.widget_classes.matches_program(program).map_err(crate::save::SaveError::Incompatible)?{return Err(crate::save::SaveError::Incompatible("Widget class admission differs from frozen Save15 program".into()));}
        use crate::save::SaveError;
        crate::save::consumer_checkpoint::preflight_program(program)?;
        crate::save::consumer_checkpoint::preflight_serialized(&self.script,16*1024*1024)?;
        self.consumer_program.validate_emitted(&self.script).map_err(|e|SaveError::Incompatible(e.to_string()))?;
        let binding=crate::consumer_schedule::ConsumerSourceBinding::from_program(program).map_err(|e|SaveError::Incompatible(e.to_string()))?;
        if self.consumer_program.source.as_ref()!=Some(&binding){return Err(SaveError::Incompatible("Save15 programme is not the current frozen Engine source binding".into()));}Ok(())
    }
    /// Capture is explicit and immutable. Scheduled payload never enters Save1–14.
    // A scalar Game clock transaction may have no evaluator computation at all.
    // Its real operation/owner remains pending until the existing debug publish.
    fn standalone_frame_entry(&self, session: &DebugSession) -> Result<bool, crate::save::SaveError> {
        let game_frame = match &session.operation {
            DebugOperation::GameFrame { .. } => true,
            DebugOperation::RestoredFrame(frame) => frame.is_game_frame(),
            _ => false,
        };
        if !game_frame || self.state.scene_execution.is_some() || self.narrative.is_some() {
            return Ok(false);
        }
        self.validate_standalone_frame_owner(&self.state)?;
        Ok(session.entry_stop && session.continuation.is_none() && session.stopped_visit.is_none()
            && session.last_site.is_none() && session.last_origin.is_none() && session.work == (0, 0)
            && session.armed_step.is_none() && !session.complete && session.error.is_none()
            && Probe::empty_complete(&session.probe))
    }
    pub fn capture_save15(&self,program:&rvn_parser::ParsedProgram,session:Option<&DebugSession>,slot:u32,label:String)->Result<crate::save::consumer_checkpoint::Save15,crate::save::SaveError>{self.capture_save15_inner(program,session,slot,label,None,false)}
    /// Read-only capture while the real host keeps all Source readers fenced.
    /// This data never substitutes for live preparation or a publication guard.
    pub fn capture_save15_with_fenced_media(&self,program:&rvn_parser::ParsedProgram,session:Option<&DebugSession>,slot:u32,label:String,snapshot:crate::audio_checkpoint::AudioCheckpointSnapshot,native:Vec<rvn_media::checkpoint::NativeCheckpoint>)->Result<crate::save::consumer_checkpoint::Save15,crate::save::SaveError>{
        self.capture_save15_inner(program,session,slot,label,Some((snapshot,native)),false)
    }
    pub fn save15_session_requires_frame_audio(&self,session:Option<&DebugSession>)->bool{session.is_some_and(|s|matches!(s.operation,DebugOperation::GameFrame{..}|DebugOperation::HostFrame{..}|DebugOperation::RestoredFrame(_)))}
    fn capture_startup_save15_inner(&self,program:&rvn_parser::ParsedProgram,session:Option<&DebugSession>,slot:u32,label:String,fenced:Option<(crate::audio_checkpoint::AudioCheckpointSnapshot,Vec<rvn_media::checkpoint::NativeCheckpoint>)>)->Result<crate::save::consumer_checkpoint::Save15,crate::save::SaveError>{self.capture_save15_inner(program,session,slot,label,fenced,true)}
    fn capture_save15_inner(&self,program:&rvn_parser::ParsedProgram,session:Option<&DebugSession>,slot:u32,label:String,fenced:Option<(crate::audio_checkpoint::AudioCheckpointSnapshot,Vec<rvn_media::checkpoint::NativeCheckpoint>)>,all_startup_sources:bool)->Result<crate::save::consumer_checkpoint::Save15,crate::save::SaveError>{
        use crate::save::{SaveError,consumer_checkpoint::{Save15,PortableStory,PortableConsumerProgram,PortableDebugState}};
        self.validate_save15_program(program)?;
        // Reject before any PCM fence or save DTO: a hot macro witness has no
        // lawful cold-load owner mapping. Live debug snapshots remain exact.
        if let Some(session)=session {
            if !self.accepts_debug_session(session)||session.complete||session.error.is_some()||session.armed_step.is_some(){return Err(SaveError::Incompatible("Save15 debugger needs a valid stopped real operation".into()));}
            if Probe::has_widget_macro_owner(&session.probe,session.continuation.as_ref()).map_err(|e|SaveError::Incompatible(e.to_string()))? {
                return Err(SaveError::Incompatible("Save15 cannot cold-restore a pending Widget macro owner; finish or abandon the live debug operation before saving".into()));
            }
        }
        self.widget_classes.validate_state(&self.state).map_err(SaveError::Incompatible)?;
        if self.state.game_tasks.is_some()&&self.active_timer.is_some(){return Err(SaveError::Incompatible("Historical timer has no portable Game timer descriptor; renew or cancel it before Save15".into()));}
        crate::game_tasks::validate_portable(&self.state).map_err(SaveError::Incompatible)?;
        self.validate_standalone_timer_target(&self.state).map_err(|e|SaveError::Incompatible(e.to_string()))?;
        crate::save::consumer_checkpoint::preflight_story_state(&self.state)?;
        crate::scene::NarrativeBudget::default().values(&self.persistent_vars).map_err(SaveError::Incompatible)?;
        crate::save::consumer_checkpoint::preflight_serialized(&self.persistent_vars,16*1024*1024)?;
        let text_tables=self.functions.text_resources().portable_receipt().map_err(SaveError::Incompatible)?;
        let locale=self.locale.as_ref().map(LocaleManager::portable_receipt).transpose().map_err(SaveError::Incompatible)?;
        let frame_snapshot=if let Some(session)=session.filter(|s|matches!(s.operation,DebugOperation::GameFrame{..}|DebugOperation::HostFrame{..}|DebugOperation::RestoredFrame(_))){
            if !self.accepts_debug_session(session)||session.complete||session.error.is_some()||session.armed_step.is_some(){return Err(SaveError::Incompatible("Stopped frame source is stale before PCM fence".into()));}
            let rows=if let Some((rows,_))=&fenced{rows.clone()}else{self.renderer.capture_frame_audio_checkpoint(&self.state,self.instance_id(),self.runtime_epoch()).map_err(SaveError::Incompatible)?};
            let state=crate::frame_audio_checkpoint::checkpointed_frame_state(&self.state,self.instance_id(),self.runtime_epoch(),&rows).map_err(SaveError::Incompatible)?;
            Some(crate::audio_checkpoint::PortableGameSnapshot::captured(state).map_err(SaveError::Incompatible)?)
        }else if all_startup_sources||self.startup.is_some(){
            // Pending Init may already own real Host readers. This is the same
            // all-Source fence, without manufacturing a Frame operation.
            let rows=if let Some((rows,_))=&fenced{rows.clone()}else{self.renderer.capture_frame_audio_checkpoint(&self.state,self.instance_id(),self.runtime_epoch()).map_err(SaveError::Incompatible)?};
            let state=crate::frame_audio_checkpoint::checkpointed_frame_state(&self.state,self.instance_id(),self.runtime_epoch(),&rows).map_err(SaveError::Incompatible)?;
            Some(crate::audio_checkpoint::PortableGameSnapshot::captured(state).map_err(SaveError::Incompatible)?)
        }else{None};
        let widget_values=std::sync::Arc::new(crate::widget_reference_values_0974::WidgetValueAdmission::new(&self.state,self.widget_classes.clone(),self.instance_id(),self.runtime_epoch()).map_err(SaveError::Incompatible)?);
        for value in self.persistent_vars.values(){crate::widget_reference_values_0974::reject_widget_values(value).map_err(SaveError::Incompatible)?;}
        if let Some(continuation)=session.and_then(|session|session.continuation.as_ref()){continuation.validate_widget_values(&widget_values).map_err(|error|SaveError::Incompatible(error.to_string()))?;}
        let mut debugger=if let Some(session)=session {
            if !self.accepts_debug_session(session)||session.complete||session.error.is_some()||session.armed_step.is_some(){return Err(SaveError::Incompatible("Save15 debugger needs a valid stopped real operation".into()));}
            let frame_operation=self.capture_frame_operation_from_state(&session.operation,frame_snapshot.as_ref().map(|s|s.state()).unwrap_or(&self.state))?;
            let mut startup=match &session.operation{DebugOperation::Startup(operation)=>{self.portable_startup_for_operation(operation).map_err(|e|SaveError::Incompatible(e.to_string()))?},_=>None};
            let widget_materialization=match &session.operation{DebugOperation::MaterializeWidget(operation)=>Some(self.portable_widget_materialization(operation).map_err(SaveError::Incompatible)?),_=>None};
            let host_input=match &session.operation{DebugOperation::Statement=>None,DebugOperation::Startup(_)=>None,DebugOperation::MaterializeWidget(_)=>None,DebugOperation::Input(input)=>Some(self.portable_host_input(input).map_err(SaveError::Incompatible)?),_ if frame_operation.is_some()=>None,_=>return Err(SaveError::Incompatible("Save15 stopped operation has no lifecycle continuation adapter".into()))};
            let frame_entry = self.standalone_frame_entry(session)?;
            let widget_entry=self.widget_materialization_entry(session)?;
            let continuation=session.continuation.as_ref();
            if continuation.is_none() && !frame_entry && !widget_entry {
                return Err(SaveError::Incompatible("Save15 stopped debugger has no owned computation or actual standalone frame entry".into()));
            }
            let (namespaces,map)=crate::save::consumer_checkpoint::portable_namespaces(program,&self.script,&self.consumer_program)?;
            if let Some(startup)=&mut startup{startup.remap_loop_namespaces(&map).map_err(SaveError::Incompatible)?;startup.remap_loop_origins(program,true)?;}
            Probe::validate_screen_construction(&session.probe,self).map_err(|error|SaveError::Incompatible(error.to_string()))?;
            let media_permit=self.startup_media_scope_permit(program,&session.operation)?;
            let statement_media_permit=self.statement_media_scope_permit(program,&session.operation)?;
            let host_namespaces=if let Some(media)=&media_permit{media.namespaces().clone()}else if let Some(media)=&statement_media_permit{media.namespaces().clone()}else if let Some(widget)=&widget_materialization{self.widget_materialization_namespaces(widget).map_err(SaveError::Incompatible)?}else if let Some(frame)=&frame_operation{self.frame_namespaces(frame)?}else{if let Some(host)=&host_input{self.host_input_namespaces(host).map_err(SaveError::Incompatible)?}else{self.debug_statement_screen_namespaces(&session.operation).map_err(SaveError::Incompatible)?}};
            let permit=if let Some(frame)=&frame_operation{self.frame_permit(frame)?}else{let target=host_input.as_ref().map(|input|&input.target).or_else(||widget_materialization.as_ref().map(|widget|widget.target()));target.map(|target|crate::ui_consumer::HostContinuationPermit::from_resident(&self.consumer_program,&self.state,target)).transpose().map_err(|e|SaveError::Incompatible(e.to_string()))?};
            let permit=media_permit.as_ref().map(|permit|permit as &dyn crate::ui_consumer::PortableScopePermit).or_else(||statement_media_permit.as_ref().map(|permit|permit as &dyn crate::ui_consumer::PortableScopePermit)).or_else(||permit.as_ref().map(|permit|permit as &dyn crate::ui_consumer::PortableScopePermit));
            let trace = if let Some(continuation) = continuation {
                let mut trace=Probe::portable_trace_with_scope(&session.probe,continuation,permit).map_err(|e|SaveError::Incompatible(e.to_string()))?;
                trace.remap_owned_namespaces(&map,&host_namespaces).map_err(|e|SaveError::Incompatible(e.to_string()))?;
                Some(trace)
            } else { None };
            Some(PortableDebugState{startup,frame_operation,host_input,widget_materialization,trace,namespaces,pc:session.position.pc,breakpoints:session.breakpoints.clone(),watches:session.watches.clone(),entry_stop:session.entry_stop,
                stopped:session.stopped_visit.as_ref().map(|v|crate::save::consumer_checkpoint::PortableStoppedVisit{entry:v.entry,site:v.site.clone()}),last_site:session.last_site.clone(),work:session.work})
        }else{None};
        let snapshot=match frame_snapshot{Some(snapshot)=>snapshot,None=>if let Some((rows,_))=&fenced{let state=crate::audio_checkpoint::checkpointed_state(&self.state,self.instance_id(),self.runtime_epoch(),rows).map_err(SaveError::Incompatible)?;crate::audio_checkpoint::PortableGameSnapshot::captured(state).map_err(SaveError::Incompatible)?}else{self.snapshot_for_save()?}};
        if let Some(debugger)=&mut debugger{debugger.export_widget_values(&widget_values).map_err(SaveError::Incompatible)?;}
        let native_media_checkpoint=fenced.map(|(_,native)|native);
        if let Some(native)=&native_media_checkpoint{validate_native_rows(native,&self.video_views().map_err(|e|SaveError::Incompatible(e.to_string()))?).map_err(SaveError::Incompatible)?;}
        let frame_story=debugger.as_ref().filter(|d|d.frame_operation.is_some()).map(|_|self.frame_story_snapshot(snapshot.state()));
        let mut save=Save15{format_version:15,slot,label,enum_references1025:None,source:PortableConsumerProgram::from_program(program)?,text_tables,locale,widget_classes:self.widget_classes.receipt().cloned(),native_media_checkpoint,story:PortableStory::capture_with_widget_values(frame_story.as_ref().unwrap_or(snapshot.state()),program,&self.script,&self.consumer_program,Some(&widget_values))?,persistent_context:self.persistent_vars.iter().map(|(k,v)|(k.clone(),v.clone())).collect(),debugger};
        save.seal_enum_values1025()?;save.validate_enum_resources1025(&self.functions.enum_assets1025)?;save.encode()?;Ok(save)
    }
    /// Preserve the existing explicit Save15 entry point. No Save1–14 wire or
    /// historical load path acquires a locale migration through this API.
    pub fn restore_save15(&mut self,program:&rvn_parser::ParsedProgram,save:crate::save::consumer_checkpoint::Save15)->Result<Option<DebugSession>,crate::save::SaveError>{
        let locale=self.locale.as_ref().map(LocaleManager::debugger_copy);
        let prepared=self.prepare_save15_with_locale(program,save,locale)?;
        self.commit_prepared_save15(prepared)
    }


    /// Data-only audio warming from this actual programme. The result authorizes
    /// no cursor, owner, renderer or commit; full preparation is required again.
    pub fn save15_state_for_audio_warming(&self,program:&rvn_parser::ParsedProgram,save:&crate::save::consumer_checkpoint::Save15)->Result<GameState,crate::save::SaveError>{
        use crate::save::SaveError;
        self.validate_save15_program(program)?;save.source.validate_current(program)?;save.encode()?;save.validate_enum_resources1025(&self.functions.enum_assets1025)?;
        Self::reject_macro_save15_debugger(save)?;
        if !self.widget_classes.accepts_receipt(save.widget_classes.as_ref()){return Err(SaveError::Incompatible("Saved Widget classes differ from the actual loaded class/source admission".into()));}
        if !self.functions.text_resources().accepts_portable_receipt(&save.text_tables).map_err(SaveError::Incompatible)?{return Err(SaveError::Incompatible("Audio warming tables differ from actual registry".into()));}
        if self.persistent_vars.len()!=save.persistent_context.len()||self.persistent_vars.iter().any(|(k,v)|save.persistent_context.get(k)!=Some(v)){return Err(SaveError::Incompatible("Audio warming persistent context differs".into()));}
        let mut next=save.story.restore(program,&self.script,&self.consumer_program)?;
        self.widget_classes.validate_state(&next).map_err(SaveError::Incompatible)?;
        match save.debugger.as_ref().and_then(|debugger|debugger.startup.as_ref()){
            Some(portable)=>{let mut candidate=self.debugger_candidate(Probe::shared());let startup=candidate.startup.as_mut().ok_or_else(||SaveError::Incompatible("Saved Init requires a pending actual startup owner; ordinary gameplay cannot replay Init".into()))?;startup.restore(portable,&self.script).map_err(SaveError::Incompatible)?;candidate.rebind_startup_loop_gathers(program)?;},
            None if self.startup.is_some()=>return Err(SaveError::Incompatible("Pending startup cannot restore a gameplay checkpoint as Init authority".into())),
            None=>{},
        }
        // A Frame's portable Story deliberately excludes Host instances. Use
        // the same actual recreated lifecycle admission as full preparation;
        // saved names never create a Host or authorize an audio source.
        if let Some(frame)=save.debugger.as_ref().and_then(|d|d.frame_operation.as_ref()) {
            self.merge_frame_hosts(&mut next,frame).map_err(|e|SaveError::Incompatible(format!("Audio warming Host merge: {e}")))?;
        }
        if next.story_identity!=self.state.story_identity{return Err(SaveError::Incompatible("Audio warming story differs".into()));}
        if let Some(program)=&self.narrative{let execution=next.scene_execution.as_ref().ok_or_else(||SaveError::Incompatible("Audio warming requires actual scene state".into()))?;execution.validate(program,&next.vars,&next.call_stack).map_err(SaveError::Incompatible)?;crate::scene_resources::validate_current_resource_state(&next).map_err(SaveError::Incompatible)?;}
        else if next.scene_execution.is_some()||next.pc>self.script.len()||next.call_stack.len()>128||next.call_stack.iter().any(|pc|*pc>self.script.len()){return Err(SaveError::Incompatible("Audio warming legacy positions are invalid".into()));}
        if save.debugger.as_ref().is_some_and(|d|d.frame_operation.is_some()||d.startup.is_some()){crate::frame_audio_checkpoint::require_frame_checkpoints(&next).map_err(SaveError::Incompatible)?;}
        else{crate::audio_checkpoint::require_saved_checkpoints(&next).map_err(SaveError::Incompatible)?;}Ok(next)
    }
    /// Prepare all restored views/continuations using an owned, host-verified
    /// selected locale. The current Engine, history, renderer and settings are
    /// untouched on every refusal. The caller supplies actual host authority.
    pub fn restore_save15_with_host_context(&mut self,program:&rvn_parser::ParsedProgram,save:crate::save::consumer_checkpoint::Save15,context:&super::HostLifecycleRestoreContext)->Result<Option<DebugSession>,crate::save::SaveError>{
        let locale=self.locale.as_ref().map(LocaleManager::debugger_copy);
        let prepared=self.prepare_save15_with_locale_and_host_context(program,save,locale,context)?;
        self.commit_prepared_save15(prepared)
    }
    pub fn prepare_save15_with_locale(&self,program:&rvn_parser::ParsedProgram,save:crate::save::consumer_checkpoint::Save15,locale:Option<LocaleManager>)->Result<PreparedSave15,crate::save::SaveError>{
        self.prepare_save15_scoped(program,save,locale,None)
    }
    /// The actual live lifecycle proof is checked on this Engine before an
    /// owned locale candidate or restored Host data can authorize anything.
    pub fn prepare_save15_with_locale_and_host_context(&self,program:&rvn_parser::ParsedProgram,save:crate::save::consumer_checkpoint::Save15,locale:Option<LocaleManager>,context:&super::HostLifecycleRestoreContext)->Result<PreparedSave15,crate::save::SaveError>{
        self.prepare_save15_scoped(program,save,locale,Some(context))
    }
    fn prepare_save15_scoped(&self,program:&rvn_parser::ParsedProgram,mut save:crate::save::consumer_checkpoint::Save15,locale:Option<LocaleManager>,host_context:Option<&super::HostLifecycleRestoreContext>)->Result<PreparedSave15,crate::save::SaveError>{

        use crate::save::SaveError;
        self.validate_save15_program(program)?;save.source.validate_current(program)?;save.encode()?;save.validate_enum_resources1025(&self.functions.enum_assets1025)?;
        Self::reject_macro_save15_debugger(&save)?;
        if !self.widget_classes.accepts_receipt(save.widget_classes.as_ref()){return Err(SaveError::Incompatible("Saved Widget classes differ from the actual loaded class/source admission".into()));}
        if !self.functions.text_resources().accepts_portable_receipt(&save.text_tables).map_err(SaveError::Incompatible)?{return Err(SaveError::Incompatible("Save15 StringTable content/provider differs from current registry".into()));}
        let source=self.debug_state_source().map_err(|e|SaveError::Incompatible(e.to_string()))?;
        let widget_retirement=self.prepare_widget_load_plan()?;
        let mut candidate=self.debugger_candidate(Probe::shared());
        candidate.planning_load_retirement=widget_retirement.clone();
        // Preparation validates a different saved transaction in a disposable
        // candidate. It neither transfers nor clears the live session barrier.
        candidate.restored_widget_barrier=false;
        candidate.functions.debug_probe=None;
        // The saved GameTasks snapshot replaces either timer storage. Keep the
        // historical live slot only on the original Engine until real commit.
        if candidate.state.game_tasks.is_some(){candidate.active_timer=None;}
        candidate.locale=locale;
        candidate.interface_view_cache.lock().map_err(|_|SaveError::Incompatible("Restore candidate interface cache unavailable".into()))?.clear();
        match (&candidate.locale,&save.locale){(None,None)=>{},(Some(current),Some(expected))if current.accepts_portable_receipt(expected).map_err(SaveError::Incompatible)?=>{},_=>return Err(SaveError::Incompatible("Save15 locale/catalog/provider differs from current frozen provider".into()))}
        if self.persistent_vars.len()!=save.persistent_context.len()||self.persistent_vars.iter().any(|(k,v)|save.persistent_context.get(k)!=Some(v)){return Err(SaveError::Incompatible("Save15 persistent context differs from stopped transaction".into()));}
        let mut next=save.story.restore(program,&self.script,&self.consumer_program)?;
        self.widget_classes.validate_state(&next).map_err(SaveError::Incompatible)?;
        let reference_epoch=self.runtime_epoch.checked_add(1).ok_or_else(||SaveError::Incompatible("Widget value runtime epoch exhausted".into()))?;
        let widget_values=std::sync::Arc::new(crate::widget_reference_values_0974::WidgetValueAdmission::new(&next,self.widget_classes.clone(),self.instance_id(),reference_epoch).map_err(SaveError::Incompatible)?);
        crate::widget_reference_values_0974::visit_state_values_mut(&mut next,&mut |value|widget_values.value(value,crate::widget_reference_values_0974::ReferencePhase::Hydrate)).map_err(SaveError::Incompatible)?;
        if let Some(debugger)=&mut save.debugger{debugger.hydrate_widget_values(widget_values).map_err(SaveError::Incompatible)?;}

        match save.debugger.as_ref().and_then(|debugger|debugger.startup.as_ref()){
            Some(portable)=>{let startup=candidate.startup.as_mut().ok_or_else(||SaveError::Incompatible("Saved Init requires a pending actual startup owner; ordinary gameplay cannot replay Init".into()))?;startup.restore(portable,&candidate.script).map_err(SaveError::Incompatible)?;},
            None if candidate.startup.is_some()=>return Err(SaveError::Incompatible("Pending startup cannot restore a gameplay checkpoint as Init authority".into())),
            None=>{},
        }
        candidate.rebind_startup_loop_gathers(program)?;
        if let Some(frame)=save.debugger.as_mut().and_then(|d|d.frame_operation.as_mut()){
            if let Some(context)=host_context {
                // Rebase against the original live Engine/context. Preserve
                // candidate.locale while adopting the admitted Host registry.
                let plan=self.rebase_frame_host_context(program,&next,frame,context)?;
                *frame=plan.frame;candidate.state=plan.state;
            }
            candidate.merge_frame_hosts(&mut next,frame).map_err(|e|SaveError::Incompatible(format!("Host merge: {e}")))?;
        }else if host_context.is_some(){return Err(SaveError::Incompatible("Host context requires an actual stopped frame continuation".into()));}
        let frame_audio=save.debugger.as_ref().is_some_and(|d|d.frame_operation.is_some()||d.startup.is_some());
        if frame_audio{self.renderer.validate_frame_audio_restore(&next).map_err(SaveError::Incompatible)?;}

        let logical=next.clone();
        if next.story_identity!=self.state.story_identity{return Err(SaveError::Incompatible("Save15 story identity differs from loaded frozen programme".into()));}

        // Scene restore uses existing exact resource/activation/audio/render
        // preflight. Legacy requires the same atomic batch renderer boundary.
        let mut batch=if self.narrative.is_some(){candidate.scene_restore_batch(next)?}else{
            if next.scene_execution.is_some()||next.pc>self.script.len()||next.call_stack.len()>128||next.call_stack.iter().any(|pc|*pc>self.script.len()){return Err(SaveError::Incompatible("Invalid legacy Save15 position/stack".into()));}
            if next.vars.len()>4096{return Err(SaveError::Incompatible("Save15 variable count exceeds historical player bound".into()));}
            next.motions.validate().map_err(SaveError::Incompatible)?;next.videos.validate().map_err(SaveError::Incompatible)?;
            if next.motions.waiting.is_some() && !matches!(Self::consumer_leaf(self.script.get(next.pc)),Some(Statement::MotionWait{..})) {
                return Err(SaveError::Incompatible("Save15 animation wait has no matching actual instruction".into()));
            }
            if let Some(alias)=&next.videos.waiting {
                if !next.videos.tracks[alias].clip.cinematic && !candidate.startup_video_wait_matches(&next) && !matches!(Self::consumer_leaf(next.pc.checked_sub(1).and_then(|pc|self.script.get(pc))),Some(Statement::VideoWait{..})) {
                    return Err(SaveError::Incompatible("Save15 video wait has no matching actual instruction".into()));
                }
            }
            crate::scene::NarrativeBudget::default().values(&next.vars).map_err(SaveError::Incompatible)?;
            crate::scene_resources::validate_current_resource_state(&next).map_err(SaveError::Incompatible)?;
            crate::display_reveal::validate_state(&next).map_err(SaveError::Incompatible)?;
            crate::audio_checkpoint::require_saved_checkpoints(&next).map_err(SaveError::Incompatible)?;
            self.renderer.validate_audio_restore(&next).map_err(SaveError::Incompatible)?;
            let runtime=self.runtime_epoch.checked_add(1).ok_or_else(||SaveError::Incompatible("Runtime epoch exhausted".into()))?;
            let interface=self.interface_epoch.checked_add(1).ok_or_else(||SaveError::Incompatible("Interface epoch exhausted".into()))?;
            let video=self.video_epoch.checked_add(1).ok_or_else(||SaveError::Incompatible("Video epoch exhausted".into()))?;
            self.display_generation.checked_add(1).ok_or_else(||SaveError::Incompatible("Display generation exhausted".into()))?;
            let batch=candidate.build_scene_batch(next,runtime,interface,video).map_err(|e|SaveError::Incompatible(e.to_string()))?;
            self.renderer.validate_scene_batch(&batch).map_err(SaveError::Incompatible)?;batch
        };
        // The saved data never supplies a host image/input receipt. Rebuild
        // only the exact source-owned wait projection for a fresh publication;
        // the real backend must install it before issuing a new input source.
        if candidate.startup.as_ref().is_some_and(|startup|startup.waiting_imagemap()){
            batch.display=candidate.startup_imagemap_interaction().map_err(|error|SaveError::Incompatible(error.to_string()))?;
            self.renderer.validate_scene_batch(&batch).map_err(SaveError::Incompatible)?;
        }
        if let Some(native)=&save.native_media_checkpoint{validate_native_rows(native,&batch.videos).map_err(SaveError::Incompatible)?;}
        let native_media=save.native_media_checkpoint.take();
        candidate.state=batch.state.clone();candidate.runtime_epoch=batch.runtime_epoch;candidate.interface_epoch=batch.interface_epoch;candidate.video_epoch=batch.video_epoch;
        let (session,startup_wait_configuration)=if let Some(mut portable)=save.debugger {
            let (actual,map)=crate::save::consumer_checkpoint::portable_namespaces(program,&self.script,&self.consumer_program)?;
            if actual!=portable.namespaces{return Err(SaveError::Incompatible("Portable continuation namespace catalogue differs from loaded source plans".into()));}
            if [portable.host_input.is_some(),portable.frame_operation.is_some(),portable.startup.is_some(),portable.widget_materialization.is_some()].into_iter().filter(|present|*present).count()>1{return Err(SaveError::Incompatible("Saved continuation cannot combine independently owned operation authorities".into()));}
            if portable.pc!=candidate.state.pc{return Err(SaveError::Incompatible("Saved debugger operation differs from player PC".into()));}
            if candidate.startup.as_ref().is_some_and(|startup|startup.waiting_imagemap()||startup.is_waiting_video())&&!portable.startup.as_ref().is_some_and(|startup|startup.has_video_callback()){
                let operation=if candidate.startup.as_ref().is_some_and(|startup|startup.waiting_imagemap()){
                    candidate.validate_startup_imagemap_gathers().map_err(|error|SaveError::Incompatible(error.to_string()))?;
                    candidate.validate_startup_imagemap_wait()
                }else{candidate.validate_startup_video_wait()}.map_err(|error|SaveError::Incompatible(error.to_string()))?;
                if portable.trace.is_some()||portable.last_site.as_ref()!=Some(&operation.entry_site()){
                    return Err(SaveError::Incompatible("Executed Init wait cannot hydrate a pending expression or a forged site".into()));
                }
                let mut breakpoints=portable.breakpoints;
                for breakpoint in &mut breakpoints{breakpoint.position=candidate.narrative.as_ref().map(|program|program.position(breakpoint.position.pc)).unwrap_or(crate::scene::ResumePosition{pc:breakpoint.position.pc,origin:None});}
                candidate.validate_debug_breakpoints(&breakpoints).map_err(|error|SaveError::Incompatible(error.to_string()))?;
                if portable.watches.len()>wire::MAX_WATCHES||portable.watches.iter().any(|watch|!rvn_parser::is_variable_binding_name(&watch.name)||watch.name.len()>4096){return Err(SaveError::Incompatible("Invalid saved waiting Init watches".into()));}
                (None,Some((breakpoints,portable.watches)))
            }else if portable.trace.is_none() && portable.widget_materialization.is_some(){
                if !portable.is_widget_materialization_entry_phase(){return Err(SaveError::Incompatible("Saved Widget materialization omitted a real pending computation trace".into()));}
                let operation=candidate.restore_widget_materialization(portable.widget_materialization.as_ref().unwrap()).map_err(SaveError::Incompatible)?;
                let mut session=candidate.debug_begin(DebugOperation::MaterializeWidget(operation),portable.breakpoints).map_err(|error|SaveError::Incompatible(error.to_string()))?;
                if !candidate.widget_materialization_entry(&session)?{return Err(SaveError::Incompatible("Restored Widget entry requests an actual computation; its trace cannot be omitted".into()));}
                session.set_watches(portable.watches).map_err(SaveError::Incompatible)?;
                Probe::prepare_screen_publication(&session.probe,None).map_err(|error|SaveError::Incompatible(error.to_string()))?;
                (Some(session),None)
            }else if portable.trace.is_none() && portable.frame_operation.is_some() {
                if !portable.is_frame_entry_phase() {
                    return Err(SaveError::Incompatible("Saved scalar frame is not stopped at its real operation entry".into()));
                }
                candidate.validate_standalone_frame_owner(&logical)?;
                candidate.validate_standalone_frame_owner(&candidate.state)?;
                let frame = portable.frame_operation.as_ref().unwrap();
                let operation = DebugOperation::RestoredFrame(candidate.restore_frame_operation(frame, &logical)?);
                let mut session = candidate.debug_begin(operation, portable.breakpoints)
                    .map_err(|e| SaveError::Incompatible(e.to_string()))?;
                if !candidate.standalone_frame_entry(&session)? {
                    return Err(SaveError::Incompatible("Restored frame actually requests a computation; its trace cannot be omitted".into()));
                }
                session.set_watches(portable.watches).map_err(SaveError::Incompatible)?;
                Probe::prepare_screen_publication(&session.probe, None).map_err(|e| SaveError::Incompatible(e.to_string()))?;
                (Some(session), None)
            }else{
            let mut trace=portable.trace.take().ok_or_else(||SaveError::Incompatible("A pending computation requires its actual trace".into()))?;
            let operation=if let Some(startup)=&portable.startup{DebugOperation::Startup(candidate.restore_startup_video_operation(startup,&batch.videos).map_err(|e|SaveError::Incompatible(e.to_string()))?)}else if let Some(frame)=&portable.frame_operation{DebugOperation::RestoredFrame(candidate.restore_frame_operation(frame,&logical).map_err(|e|SaveError::Incompatible(format!("Frame hydration: {e}")))?)}else if let Some(widget)=&portable.widget_materialization{DebugOperation::MaterializeWidget(candidate.restore_widget_materialization(widget).map_err(SaveError::Incompatible)?)}else if let Some(host)=&portable.host_input{DebugOperation::Input(candidate.restore_host_input(host).map_err(SaveError::Incompatible)?)}else{DebugOperation::Statement};
            // Authenticate/prepare the owned session before scoped media
            // permits borrow this disposable candidate for the entire replay.
            // Every permit and exact namespace/trace check still precedes the
            // first restored record and any publication to the live Engine.
            if portable.pc!=candidate.state.pc{return Err(SaveError::Incompatible("Saved debugger operation differs from player PC".into()));}
            let mut breakpoints=portable.breakpoints;for breakpoint in &mut breakpoints{breakpoint.position=candidate.narrative.as_ref().map(|p|p.position(breakpoint.position.pc)).unwrap_or(crate::scene::ResumePosition{pc:breakpoint.position.pc,origin:None});}
            let mut session=match &operation{
                DebugOperation::Input(input)if candidate.has_pending_widget_materialization()=>candidate.debug_begin_restored_host_input(input.clone(),breakpoints),
                _=>candidate.debug_begin(operation.clone(),breakpoints),
            }.map_err(|e|SaveError::Incompatible(format!("Frame debug begin: {e}")))?;
            let media_permit=candidate.startup_media_scope_permit(program,&operation)?;
            let statement_media_permit=candidate.statement_media_scope_permit(program,&operation)?;
            let host_namespaces=if let Some(media)=&media_permit{media.namespaces().clone()}else if let Some(media)=&statement_media_permit{media.namespaces().clone()}else if let Some(widget)=&portable.widget_materialization{candidate.widget_materialization_namespaces(widget).map_err(SaveError::Incompatible)?}else if let Some(frame)=&portable.frame_operation{candidate.frame_namespaces(frame)?}else{if let Some(host)=&portable.host_input{candidate.host_input_namespaces(host).map_err(SaveError::Incompatible)?}else{candidate.debug_statement_screen_namespaces(&operation).map_err(SaveError::Incompatible)?}};
            let host_namespaces=host_namespaces.into_iter().map(|(root,map)|(root,map.into_iter().map(|(live,logical)|(logical,live)).collect())).collect();
            let permit=if let Some(frame)=&portable.frame_operation{candidate.frame_permit(frame)?}else{let target=portable.host_input.as_ref().map(|input|&input.target).or_else(||portable.widget_materialization.as_ref().map(|widget|widget.target()));target.map(|target|crate::ui_consumer::HostContinuationPermit::from_resident(&candidate.consumer_program,&candidate.state,target)).transpose().map_err(|e|SaveError::Incompatible(e.to_string()))?};
            let map=map.into_iter().map(|(live,logical)|(logical,live)).collect();trace.remap_owned_namespaces(&map,&host_namespaces).map_err(|e|SaveError::Incompatible(e.to_string()))?;
            let permit=media_permit.as_ref().map(|permit|permit as &dyn crate::ui_consumer::PortableScopePermit).or_else(||statement_media_permit.as_ref().map(|permit|permit as &dyn crate::ui_consumer::PortableScopePermit)).or_else(||permit.as_ref().map(|permit|permit as &dyn crate::ui_consumer::PortableScopePermit));
            session.set_watches(portable.watches).map_err(SaveError::Incompatible)?;
            for record in trace.records(){let current=session.continuation.as_ref().ok_or_else(||SaveError::Incompatible("Saved trace has an unrequested completed root".into()))?;Probe::restore_record_with_scope(&session.probe,current,record,candidate.locale.as_ref(),permit).map_err(|e|SaveError::Incompatible(e.to_string()))?;candidate.debug_prepare(&mut session).map_err(|e|SaveError::Incompatible(e.to_string()))?;}
            session.continuation.as_mut().ok_or_else(||SaveError::Incompatible("Saved pending root is not actually requested".into()))?.restore_portable(&trace,candidate.locale.as_ref()).map_err(|e|SaveError::Incompatible(e.to_string()))?;
            session.entry_stop=portable.entry_stop;
            if let Some(stopped)=&portable.stopped {if stopped.entry!=session.entry_stop||stopped.site!=session.visit().site{return Err(SaveError::Incompatible("Saved breakpoint visit differs from restored actual continuation".into()));}}
            session.stopped_visit=portable.stopped.map(|v|StoppedVisit{entry:v.entry,site:v.site});session.last_site=portable.last_site;session.work=portable.work;
            Probe::prepare_screen_publication(&session.probe,session.continuation.as_ref()).map_err(|e|SaveError::Incompatible(e.to_string()))?;
            (Some(session),None)
            }
        }else{(None,None)};
        // Hydration above consumed the exact saved logical context/trace. Only
        // now record the new representation work; never replay construction
        // into the transaction which was already stopped in that save.
        Self::defer_saved_widget_materialization(&mut batch.state);
        candidate.state=batch.state.clone();
        candidate.interface_view_cache.lock().map_err(|_|SaveError::Incompatible("Deferred Widget source cache unavailable".into()))?.clear();
        self.renderer.validate_scene_batch(&batch).map_err(SaveError::Incompatible)?;
        candidate.history.clear();
        if candidate.debugger_anchor_measured().map_err(|e|SaveError::Incompatible(e.to_string()))?.1>16*1024*1024-4096{return Err(SaveError::Incompatible("Save15 restored debugger anchor lacks bounded generation headroom".into()));}
        candidate.display_generation=self.display_generation.checked_add(1).ok_or_else(||SaveError::Incompatible("Display generation exhausted".into()))?;
        candidate.runtime_epoch=batch.runtime_epoch;candidate.interface_epoch=batch.interface_epoch;candidate.video_epoch=batch.video_epoch;
        candidate.display_owner=batch.state.scene_execution.as_ref().and_then(|execution|execution.owner());candidate.display_history_entry=None;candidate.legacy_preview_visit=None;
        let reveal_source=candidate.display_reveal_source();
        let choice_backdrop=if matches!(batch.display,Some(Interaction::Choice{..})){candidate.last_dialogue_interaction().map_err(|e|SaveError::Incompatible(e.to_string()))?}else{None};
        // Target hydration has succeeded without touching old objects. Prepare
        // the real old-source retirement only now, still without publication.
        let (widget_retirement,speech)=self.prepare_widget_load_retirement_with_effects(widget_retirement)?;
        batch.retired_widget_speech=speech;
        self.renderer.validate_scene_batch(&batch).map_err(SaveError::Incompatible)?;
        Ok(PreparedSave15{source,widget_retirement,batch,locale:candidate.locale.take(),session,reveal_source,choice_backdrop,frame_audio,startup:candidate.startup.clone(),native_media,startup_wait_configuration})
    }

    /// Publish the exact prepared state and selected locale once. Rechecking
    /// source and renderer preflight precedes every live mutation.
    pub fn commit_prepared_save15(&mut self,prepared:PreparedSave15)->Result<Option<DebugSession>,crate::save::SaveError>{Ok(self.begin_prepared_save15_commit(prepared)?.commit())}
    pub fn begin_prepared_save15_commit(&mut self,prepared:PreparedSave15)->Result<Save15CommitGuard<'_,R>,crate::save::SaveError>{
        use crate::save::SaveError;
        self.renderer.validate_prepared_native_restore(&prepared).map_err(SaveError::Incompatible)?;
        if !self.accepts_debug_state_source(&prepared.source){return Err(SaveError::Incompatible("Live Engine changed after Save15 preparation".into()));}
        if prepared.frame_audio{self.renderer.validate_frame_audio_restore(&prepared.batch.state).map_err(SaveError::Incompatible)?;}
        else{self.renderer.validate_audio_restore(&prepared.batch.state).map_err(SaveError::Incompatible)?;}
        self.renderer.validate_scene_batch(&prepared.batch).map_err(SaveError::Incompatible)?;
        let widget_reservation=prepared.widget_retirement.as_ref().map(|plan|{
            self.renderer.validate_widget_viewport_batch(plan.as_ref())?;
            self.renderer.reserve_widget_viewport_batch(plan.clone())
        }).transpose().map_err(SaveError::Incompatible)?;
        Ok(Save15CommitGuard{engine:self,prepared,widget_reservation})
    }
    pub fn accepts_prepared_save15(&self,prepared:&PreparedSave15)->bool{self.accepts_debug_state_source(&prepared.source)}
    fn publish_prepared_save15(&mut self,prepared:PreparedSave15)->Option<DebugSession>{
        let PreparedSave15{source:_,widget_retirement:_,batch,locale,mut session,reveal_source:_,choice_backdrop:_,frame_audio:_,startup,native_media:_,startup_wait_configuration:_}=prepared;
        // Infallible owned publication: candidate views already use this locale.
        self.locale=locale;
        self.startup=startup;
        self.interface_view_cache.lock().unwrap_or_else(|poison|poison.into_inner()).clear();
        self.publish_scene_restore(batch);self.history.clear();self.renderer.loaded_compatibility(LoadCompatibility::Verified);
        self.restored_widget_barrier=session.is_some()&&self.has_pending_widget_materialization();
        if let Some(session)=&mut session {session.owns_materialization_barrier=self.restored_widget_barrier;session.instance=self.instance_id();session.epoch=self.runtime_epoch();session.text_resources=self.functions.text_resources().revision_stamp();session.locale_revision=self.locale.as_ref().map(LocaleManager::revision_stamp);session.anchor=self.debugger_anchor().expect("bounded candidate anchor preflighted");session.position=self.narrative.as_ref().map(|p|p.position(self.state.pc)).unwrap_or(crate::scene::ResumePosition{pc:self.state.pc,origin:None});self.publish_screen_construction_session(session);}
        session
    }
}

fn validate_native_rows(rows:&[rvn_media::checkpoint::NativeCheckpoint],views:&[crate::video::VideoView])->Result<(),String>{
    let required:Vec<_>=views.iter().filter(|view|!view.track.finished()||(view.track.playback==crate::video::Playback::Ended&&view.track.clip.keep_last_frame)).collect();
    if rows.len()!=required.len()||rows.len()>8{return Err("Native checkpoint does not cover actual Core video views".into());}
    let mut ids=std::collections::HashSet::new();for row in rows{row.validate()?;if !ids.insert(&row.id)||!required.iter().any(|view|view.id==row.id&&view.track.clip.source==row.source_locator&&view.track.clip.mask.as_ref()==row.mask.as_ref().map(|m|&m.source_locator)&&(!view.track.finished()||row.audio.is_none())){return Err("Native checkpoint source/owner is not an admitted resumable Core video".into());}}Ok(())
}

#[derive(serde::Serialize)]
enum LegacyIntent {
 Background(String,Transition),Cinematic(Option<String>,Option<String>),Ending(String),Typewriter(f32),
 Show(String,Option<String>,Position,Transition,Option<SpriteState>),Hide(String,Transition,SpriteState),Move(String,Position,Transition,SpriteState),
 Animate(String,String,Vec<rvn_parser::AnimationParam>),StopAnimation(String),Effect(String,Option<bool>,Option<bool>,Option<f32>,Option<f32>,Option<String>)
}
#[derive(Default)]
pub(super) struct LegacyIntents { entries:Vec<LegacyIntent>,bytes:usize,error:bool }
impl LegacyIntents {
 fn record<T:serde::Serialize+?Sized>(&mut self,check:&T,build:impl FnOnce()->LegacyIntent){
  if self.error{return;}
  struct Bound(usize);
  impl std::io::Write for Bound {
   fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{self.0=self.0.checked_add(bytes.len()).filter(|n|*n<=64*1024).ok_or_else(||std::io::Error::other("Classic renderer intent budget"))?;Ok(bytes.len())}
   fn flush(&mut self)->std::io::Result<()>{Ok(())}
  }
  let mut bound=Bound(self.bytes.saturating_add(64));
  if self.entries.len()>=128||serde_json::to_writer(&mut bound,check).is_err(){self.error=true;return;}
  self.bytes=bound.0;self.entries.push(build());
 }
 pub(super) fn validate(&self)->Result<(),String>{if self.error{Err("Classic renderer intents exceed 128 commands or 64 KiB".into())}else{Ok(())}}
 // Scene state already represents visual legacy commands during Startup.
 // Only persistent host one-shots need to cross its publication fence.
 pub(super) fn into_startup_effects(mut self)->Self{self.entries.retain(|intent|matches!(intent,LegacyIntent::Ending(_)));self}
 pub(super) fn is_empty(&self)->bool{self.entries.is_empty()}
 pub(super) fn publish<R:Renderer>(&mut self,renderer:&mut R){for intent in self.entries.drain(..){match intent {
  LegacyIntent::Background(path,t)=>renderer.set_background(&path,&t),
  LegacyIntent::Cinematic(Some(id),t)=>renderer.show_cinematic(&id,t.as_deref()),LegacyIntent::Cinematic(None,t)=>renderer.hide_cinematic(t.as_deref()),
  LegacyIntent::Ending(id)=>renderer.unlock_ending(&id),LegacyIntent::Typewriter(speed)=>renderer.set_typewriter_config(speed),
  LegacyIntent::Show(id,e,p,t,f)=>renderer.show_sprite(&id,e.as_deref(),&p,&t,f.as_ref()),LegacyIntent::Hide(id,t,f)=>renderer.hide_sprite(&id,&t,&f),LegacyIntent::Move(id,p,t,f)=>renderer.move_sprite(&id,&p,&t,&f),
  LegacyIntent::Animate(id,a,p)=>renderer.animate_sprite(&id,&a,&p),LegacyIntent::StopAnimation(id)=>renderer.stop_sprite_animation(&id),
  LegacyIntent::Effect(id,x,y,s,r,t)=>renderer.set_sprite_effect(&id,x,y,s,r,t.as_deref()),
 }}}
}
impl<R:Renderer> PlanningRenderer<'_,R>{fn legacy<T:serde::Serialize+?Sized>(&mut self,check:&T,build:impl FnOnce()->LegacyIntent){if let Some(target)=&self.2{target.borrow_mut().record(check,build);}}}
