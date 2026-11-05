//! Owned preparation for the actual startup owner and host publication fence.
use super::*;
#[cfg(test)]
#[path="engine_startup_step_tests.rs"]
mod tests;
/// Read-only observation of one actual owner/source/continuation. This carries
/// no executable client payload and cannot authorize publication or a step.
pub struct StartupObservationSource{engine:DebugStateSource,session:Option<[u8;32]>,configuration:[u8;32]}
impl<R:Renderer> EngineStartup<R>{
    fn observation_configuration(&self)->Result<[u8;32],RuntimeError>{
        use std::fmt::Write;
        let mut hash=Fingerprint{hash:Sha256::new(),bytes:0};
        write!(&mut hash,"{:?}{:?}",self.breakpoints,self.watches).map_err(|_|self.engine.scene_resource_error("Startup configuration observation exceeds its bounded source".into()))?;
        Ok(hash.hash.finalize().into())
    }
    pub fn observation_source(&self)->Result<StartupObservationSource,RuntimeError>{
        if self.session.as_ref().is_some_and(|session|!self.engine.accepts_debug_session(session)){return Err(self.engine.scene_resource_error("Startup observation has a stale actual continuation".into()));}
        Ok(StartupObservationSource{engine:self.engine.debug_state_source()?,session:self.session.as_ref().map(DebugSession::startup_step_anchor).transpose().map_err(|e|self.engine.scene_resource_error(e))?,configuration:self.observation_configuration()?})
    }
    pub fn accepts_observation_source(&self,source:&StartupObservationSource)->bool{
        self.engine.accepts_debug_state_source(&source.engine)&&self.session.as_ref().map(DebugSession::startup_step_anchor).transpose().is_ok_and(|session|session==source.session)
            &&self.observation_configuration().is_ok_and(|configuration|configuration==source.configuration)
    }
}

/// No constructor/serialization: only an admitted startup can prepare this step.
pub struct PreparedStartupStep {
    source:DebugStateSource,
    // Only an actual completed input image may bind a no-session wait step.
    // This capability is transient and never part of Save15.
    imagemap_source:Option<std::sync::Arc<StartupObservationSource>>,
    session_anchor:Option<[u8;32]>,
    progress:DebugProgress,
    batch:Option<crate::renderer::SceneRenderBatch>,
    host_effects:StartupHostEffects,
    computation_state:Option<GameState>,
    video_projection:Vec<crate::video::VideoView>,
    video_commands:Vec<StartupVideoCommand>,
    next_step_kind:Option<StepKind>,
    terminal_completion:Option<terminal::AcceptedStartupVideoTerminal>,
    snapshot:wire::Snapshot,
    session:Option<DebugSession>,
    startup:Option<StartupState>,
    persistent:HashMap<String,Value>,
    input:InputState,
    timer:Option<(f32,String,f32)>,
    host_random:host_random::HostRandomState,
    history:RollbackHistory,
    locale:Option<LocaleManager>,
}
impl PreparedStartupStep {
    /// The actual candidate's command continuation. An unvisited next Init
    /// statement is distinct from a real debugger stop at that statement.
    /// This projection never grants publication authority by itself.
    pub fn next_step_kind(&self)->Option<StepKind>{self.next_step_kind}
    pub fn waiting_for_video(&self)->bool{self.startup.as_ref().is_some_and(|startup|startup.waiting_video.is_some())}
    pub fn waiting_for_imagemap(&self)->bool{self.startup.as_ref().is_some_and(StartupState::waiting_imagemap)}
    /// A prepared reached Init wait only; arbitrary batch.display is not an
    /// input capability. The host still stages and installs the actual image.
    pub fn imagemap_interaction(&self)->Option<&Interaction>{
        if !self.waiting_for_imagemap(){return None;}
        self.batch.as_ref()?.display.as_ref().filter(|display|matches!(display,Interaction::Imagemap{..}))
    }
    pub fn video_commands(&self)->&[StartupVideoCommand]{&self.video_commands}
    pub fn render_batch(&self)->Option<&crate::renderer::SceneRenderBatch>{self.batch.as_ref()}
    pub fn snapshot(&self)->&wire::Snapshot{&self.snapshot}
    pub fn progress(&self)->DebugProgress{self.progress}
}
/// Exclusive owner borrow spans the actual host's resource/source/PCM fences.
pub struct StartupStepCommitGuard<'a,R:Renderer>{owner:&'a mut EngineStartup<R>,prepared:Box<PreparedStartupStep>}
impl<R:Renderer> StartupStepCommitGuard<'_,R>{
    pub fn render_batch(&self)->Option<&crate::renderer::SceneRenderBatch>{self.prepared.batch.as_ref()}
    pub fn snapshot(&self)->&wire::Snapshot{&self.prepared.snapshot}
    pub fn progress(&self)->DebugProgress{self.prepared.progress}
    pub fn commit(self)->DebugProgress{self.owner.publish_step_boxed(self.prepared)}
    pub fn commit_with_audio_checkpoint(mut self,snapshot:crate::audio_checkpoint::AudioCheckpointSnapshot)->Result<DebugProgress,RuntimeError>{
        self.apply_audio_checkpoint(snapshot)?;
        Ok(self.owner.publish_step_boxed(self.prepared))
    }
    fn apply_audio_checkpoint(&mut self,snapshot:crate::audio_checkpoint::AudioCheckpointSnapshot)->Result<(),RuntimeError>{
        let batch=self.prepared.batch.as_ref().ok_or_else(||self.owner.engine.scene_resource_error("A computation-only startup step has no PCM publication".into()))?;
        let next=crate::frame_audio_checkpoint::checkpointed_frame_state(&batch.state,self.owner.engine.instance_id(),batch.runtime_epoch,&snapshot).map_err(|e|self.owner.engine.scene_resource_error(e))?;
        // The measured candidate is the exact state published under the fence,
        // including retained startup state, rather than the outgoing cursor.
        let mut candidate=self.owner.engine.debugger_candidate(Probe::shared());
        candidate.functions.debug_probe=None;candidate.state=next.clone();candidate.startup=self.prepared.startup.clone();
        candidate.runtime_epoch=batch.runtime_epoch;candidate.interface_epoch=batch.interface_epoch;candidate.video_epoch=batch.video_epoch;
        candidate.locale=self.prepared.locale.as_ref().map(LocaleManager::debugger_copy);
        candidate.persistent_vars=self.prepared.persistent.clone();candidate.input_state=self.prepared.input.clone();candidate.active_timer=self.prepared.timer.clone();candidate.host_random=self.prepared.host_random.clone();candidate.history=self.prepared.history.debugger_copy();
        candidate.interface_view_cache.lock().map_err(|_|self.owner.engine.scene_resource_error("Startup fenced interface cache unavailable".into()))?.clear();
        if candidate.debugger_anchor_measured()?.1>16*1024*1024-4096{return Err(self.owner.engine.scene_resource_error("Startup fenced anchor lacks generation headroom".into()));}
        // PCM changes the next session's real Engine anchor, never its tasks.
        if let Some(session)=&mut self.prepared.session{session.anchor=candidate.debugger_anchor()?;}
        if let Some(terminal)=&mut self.prepared.terminal_completion{
            terminal.post=std::sync::Arc::new(StartupObservationSource{
                engine:candidate.debug_state_source()?,
                session:self.prepared.session.as_ref().map(DebugSession::startup_step_anchor).transpose().map_err(|error|self.owner.engine.scene_resource_error(error))?,
                configuration:self.owner.observation_configuration()?,
            });
        }
        drop(candidate);
        self.prepared.batch.as_mut().unwrap().state=next;
        Ok(())
    }
}
/// Issued only from the admitted owner, with its full live source/trace anchors.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct StartupPublicationSource{instance:u64,epoch:u64,source:[u8;32],session:Option<[u8;32]>}
impl StartupPublicationSource{
    pub fn instance_id(&self)->u64{self.instance}
    pub fn runtime_epoch(&self)->u64{self.epoch}
}
/// A narrowly scoped renderer boundary for bootstrap's real World owner.
/// Preparation may reserve transient identities but cannot publish effects.
/// Publication is infallible and transfers owned intents exactly once.
/// A bounded one-shot queue from the actual prepared Startup. No public
/// constructor, Clone or serialized representation can forge this payload.
/// Hosts consume it after replacing their batch and before draining commands.
pub struct StartupHostEffects{intents:LegacyIntents}
impl StartupHostEffects{
    fn empty()->Self{Self{intents:LegacyIntents::default()}}
    pub fn is_empty(&self)->bool{self.intents.is_empty()}
    pub fn publish<R:Renderer>(mut self,renderer:&mut R){self.intents.publish(renderer);}
}
pub trait StartupBatchSink:Renderer{
    type PreparedPublication;
    type Publication;
    /// An existing host must opt into ordered one-shot delivery. Default
    /// refusal preserves atomicity rather than silently dropping effects.
    fn validate_startup_effects(&self,_source:&StartupPublicationSource,effects:&StartupHostEffects)->Result<(),String>{
        if effects.is_empty(){Ok(())}else{Err("Startup host has no prepared one-shot publication adapter".into())}
    }
    fn publish_startup_publication_with_effects(&mut self,source:StartupPublicationSource,batch:crate::renderer::SceneRenderBatch,prepared:Self::PreparedPublication,effects:StartupHostEffects)->Self::Publication{
        assert!(effects.is_empty(),"Host must implement ordered startup effects after opting in");
        self.publish_startup_publication(source,batch,prepared)
    }
    fn prepare_startup_publication(&self,source:&StartupPublicationSource,batch:&crate::renderer::SceneRenderBatch)->Result<Self::PreparedPublication,String>;
    fn publish_startup_publication(&mut self,source:StartupPublicationSource,batch:crate::renderer::SceneRenderBatch,prepared:Self::PreparedPublication)->Self::Publication;
}
pub struct StartupHostStepGuard<'a,R:StartupBatchSink>{inner:StartupStepCommitGuard<'a,R>,source:StartupPublicationSource,publication:Option<R::PreparedPublication>}
pub struct CommittedStartupStep<P>{pub progress:DebugProgress,pub publication:Option<P>,pub terminal:Option<terminal::AcceptedStartupVideoTerminal>}
impl<R:StartupBatchSink> StartupHostStepGuard<'_,R>{
    pub fn video_commands(&self)->&[StartupVideoCommand]{self.inner.prepared.video_commands()}
    pub fn publication_source(&self)->&StartupPublicationSource{&self.source}
    pub fn host_publication(&self)->Option<&R::PreparedPublication>{self.publication.as_ref()}
    pub fn render_batch(&self)->Option<&crate::renderer::SceneRenderBatch>{self.inner.render_batch()}
    pub fn snapshot(&self)->&wire::Snapshot{self.inner.snapshot()}
    pub fn progress(&self)->DebugProgress{self.inner.progress()}
    pub fn commit_computation(self)->Result<CommittedStartupStep<R::Publication>,RuntimeError>{
        if self.inner.prepared.batch.is_some(){return Err(self.inner.owner.engine.scene_resource_error("A publishing startup step requires the actual PCM fence".into()));}
        let terminal=self.inner.prepared.terminal_completion.clone();
        Ok(CommittedStartupStep{progress:self.inner.commit(),publication:None,terminal})
    }
    pub fn commit_with_audio_checkpoint(mut self,snapshot:crate::audio_checkpoint::AudioCheckpointSnapshot)->Result<CommittedStartupStep<R::Publication>,RuntimeError>{
        self.inner.apply_audio_checkpoint(snapshot)?;
        let progress=self.inner.prepared.progress;
        let terminal=self.inner.prepared.terminal_completion.clone();
        let (batch,effects)=self.inner.owner.install_step_boxed(self.inner.prepared);
        let publication=batch.map(|batch|self.inner.owner.engine.renderer.publish_startup_publication_with_effects(self.source,batch,self.publication.expect("guard owns preflighted publication"),effects));
        Ok(CommittedStartupStep{progress,publication,terminal})
    }
}
impl<R:StartupBatchSink> EngineStartup<R>{
    /// Read-only capability admission for the real host's media preparation.
    /// Labels or batch digests alone never authorize a suspended continuation.
    pub fn prepared_publication_source(&self,prepared:&PreparedStartupStep)->Result<StartupPublicationSource,RuntimeError>{
        self.validate_prepared_step(prepared)?;
        Ok(StartupPublicationSource{instance:prepared.source.instance,epoch:prepared.source.epoch,source:prepared.source.anchor,session:prepared.session_anchor})
    }
    pub fn begin_prepared_host_step_commit(&mut self,prepared:PreparedStartupStep)->Result<StartupHostStepGuard<'_,R>,RuntimeError>{
        self.begin_prepared_host_step_commit_boxed(Box::new(prepared))
    }
    /// Retain the original uniquely owned Core candidate through the host and
    /// PCM fences. Source/session/renderer validation is the SAME shared path;
    /// heap transport is not another publication or a bypass of admission.
    #[inline(never)]
    pub fn begin_prepared_host_step_commit_boxed(&mut self,prepared:Box<PreparedStartupStep>)->Result<StartupHostStepGuard<'_,R>,RuntimeError>{
        let source=self.prepared_publication_source(&prepared)?;
        self.engine.renderer.validate_startup_effects(&source,&prepared.host_effects).map_err(|error|self.engine.scene_resource_error(error))?;
        let inner=self.begin_prepared_step_commit_boxed(prepared)?;
        let publication=inner.prepared.batch.as_ref().map(|batch|inner.owner.engine.renderer.prepare_startup_publication(&source,batch)).transpose().map_err(|e|inner.owner.engine.scene_resource_error(e))?;
        Ok(StartupHostStepGuard{inner,source,publication})
    }
}
impl<R:Renderer> EngineStartup<R>{
    /// Existing debugger/evaluator semantics on an isolated bounded trusted fork.
    #[inline(never)]
    pub fn prepare_step(&self,kind:StepKind)->Result<PreparedStartupStep,RuntimeError>{
        self.prepare_step_boxed(kind).map(|prepared|*prepared)
    }
    /// Pointer-sized ownership transfer for a host that retains this exact
    /// prepared token. All source, trace, effect and renderer admission is shared
    /// with prepare_step; this changes storage, never the owner or evaluator.
    #[inline(never)]
    pub fn prepare_step_boxed(&self,kind:StepKind)->Result<Box<PreparedStartupStep>,RuntimeError>{
        if self.waiting_for_video(){return Err(self.engine.scene_resource_error("Executed initializer video wait requires its actual media feedback".into()));}
        if self.waiting_for_imagemap(){return Err(self.engine.scene_resource_error("Executed initializer Imagemap requires its actual published input owner".into()));}
        let session=self.session.as_ref().ok_or_else(||self.engine.scene_resource_error("Startup is already complete".into()))?;
        if !self.engine.accepts_debug_session(session){return Err(self.engine.scene_resource_error("Startup source changed before step preparation".into()));}
        let source=self.engine.debug_state_source()?;
        let session_anchor=Some(session.startup_step_anchor().map_err(|e|self.engine.scene_resource_error(e))?);
        let cursor_before=self.engine.startup.as_ref().map(|startup|startup.position_key());
        let session=session.fork_startup().map_err(|e|self.engine.scene_resource_error(e))?;
        let recorded=std::rc::Rc::new(std::cell::RefCell::new(None));
        if self.engine.startup_video_trace.is_some(){return Err(self.engine.scene_resource_error("A live startup cannot own a candidate action observer".into()));}
        let video_trace=std::sync::Arc::new(Mutex::new(video_commands::StartupVideoTrace::default()));
        let effects=std::rc::Rc::new(std::cell::RefCell::new(LegacyIntents::default()));
        let mut candidate=self.engine.debugger_candidate(Probe::shared());candidate.functions.debug_probe=None;candidate.renderer.1=Some(recorded.clone());candidate.renderer.2=Some(effects.clone());
        candidate.startup_video_trace=Some(video_trace.clone());
        let mut candidate=EngineStartup{engine:candidate,program:self.program.clone(),session:Some(session),breakpoints:self.breakpoints.clone(),watches:self.watches.clone(),video_projection:self.video_projection.clone()};
        let progress=candidate.advance_step(kind)?;
        let snapshot=candidate.snapshot();
        let terminal_completion=candidate.completed_terminal_receipt(self)?;
        let batch=recorded.borrow_mut().take();
        effects.borrow().validate().map_err(|error|self.engine.scene_resource_error(error))?;
        let host_effects=StartupHostEffects{intents:std::mem::take(&mut *effects.borrow_mut()).into_startup_effects()};
        if batch.is_none()&&!host_effects.is_empty(){return Err(self.engine.scene_resource_error("Startup host effects require their actual publishing batch".into()));}
        if let Some(batch)=&batch{self.engine.renderer.validate_scene_batch(batch).map_err(|e|self.engine.scene_resource_error(e))?;}
        if candidate.engine.debugger_anchor_measured()?.1>16*1024*1024-4096{return Err(self.engine.scene_resource_error("Prepared startup anchor lacks generation headroom".into()));}
        let computation_state=if batch.is_none()&&cursor_before!=candidate.engine.startup.as_ref().map(|startup|startup.position_key()){Some(candidate.engine.state.clone())}else{None};
        let video_projection=batch.as_ref().map(|batch|batch.videos.clone()).unwrap_or_else(||self.video_projection.clone());
        let video_commands=video_trace.lock().map_err(|_|self.engine.scene_resource_error("Prepared startup video observation unavailable".into()))?.snapshot().map_err(|error|self.engine.scene_resource_error(error))?;
        let next_step_kind=if progress==DebugProgress::Running
            ||kind==StepKind::Continue&&progress==DebugProgress::Stopped&&(candidate.waiting_for_video()||candidate.waiting_for_imagemap()||candidate.session.as_ref().is_some_and(|session|session.entry_stop&&session.stopped_visit.is_none())){
            Some(kind)
        }else{None};
        Ok(Box::new(PreparedStartupStep{source,imagemap_source:None,session_anchor,progress,batch,host_effects,computation_state,video_projection,video_commands,next_step_kind,terminal_completion,snapshot,session:candidate.session.take(),startup:candidate.engine.startup.clone(),
            persistent:candidate.engine.persistent_vars.clone(),input:candidate.engine.input_state.clone(),timer:candidate.engine.active_timer.clone(),host_random:candidate.engine.host_random.clone(),history:candidate.engine.history.debugger_copy(),locale:candidate.engine.locale.take()}))
    }
    /// The completed Init owner prepares its genuine gameplay entry through
    /// the existing transfer/renderer rules. No authored Statement is executed.
    /// None is the real absence of an evaluator session, not a fabricated PC.
    #[inline(never)]
    pub fn prepare_gameplay_entry(&self,label:Option<&str>)->Result<PreparedStartupStep,RuntimeError>{
        self.prepare_gameplay_entry_boxed(label).map(|prepared|*prepared)
    }
    /// The same completed-owner entry candidate, retained without a by-value
    /// intermediate in the host's publication frame.
    #[inline(never)]
    pub fn prepare_gameplay_entry_boxed(&self,label:Option<&str>)->Result<Box<PreparedStartupStep>,RuntimeError>{
        if !self.is_complete()||self.session.is_some(){return Err(self.engine.scene_resource_error("Only completed initialization can prepare gameplay entry".into()));}
        let source=self.engine.debug_state_source()?;
        let recorded=std::rc::Rc::new(std::cell::RefCell::new(None));
        let mut candidate=self.engine.debugger_candidate(Probe::shared());candidate.functions.debug_probe=None;candidate.renderer.1=Some(recorded.clone());
        if let Some(label)=label{candidate.enter_label(label)?;}
        // Legacy/no-label entry also transfers one real complete projection.
        // It is built from the computed candidate; no authored statement runs.
        let batch=Some(match recorded.borrow_mut().take(){
            Some(batch)=>batch,
            None=>candidate.build_scene_batch(candidate.state.clone(),candidate.runtime_epoch,candidate.interface_epoch,candidate.video_epoch)?,
        });
        if let Some(batch)=&batch{self.engine.renderer.validate_scene_batch(batch).map_err(|error|self.engine.scene_resource_error(error))?;}
        if candidate.debugger_anchor_measured()?.1>16*1024*1024-4096{return Err(self.engine.scene_resource_error("Gameplay entry anchor lacks bounded headroom".into()));}
        let snapshot=candidate.debug_current_snapshot(&self.watches).map_err(|error|self.engine.scene_resource_error(error))?;
        let video_projection=batch.as_ref().map(|batch|batch.videos.clone()).unwrap_or_else(||self.video_projection.clone());
        let computation_state=batch.is_none().then(||candidate.state.clone());
        Ok(Box::new(PreparedStartupStep{source,imagemap_source:None,session_anchor:None,progress:DebugProgress::Complete,batch,host_effects:StartupHostEffects::empty(),computation_state,video_projection,video_commands:Vec::new(),next_step_kind:None,terminal_completion:None,snapshot,session:None,startup:None,
            persistent:candidate.persistent_vars.clone(),input:candidate.input_state.clone(),timer:candidate.active_timer.clone(),host_random:candidate.host_random.clone(),history:candidate.history.debugger_copy(),locale:candidate.locale.take()}))
    }
    /// Prepare only selection of the actual frozen hotspot route. It never
    /// runs the selected child, repeats the header gather, or asks Renderer for
    /// a synthetic answer. Removal of the completed input image uses the same
    /// guarded full publication as every other real Startup transition.
    #[inline(never)]
    pub fn prepare_imagemap_selection_boxed(&self,input:&StartupImagemapSource,selected:usize)->Result<Box<PreparedStartupStep>,RuntimeError>{
        if !self.accepts_imagemap_input_source(input){return Err(self.engine.scene_resource_error("Init selection is not from its retained completed input image".into()));}
        self.engine.validate_startup_imagemap_wait()?;
        self.engine.validate_startup_imagemap_gathers()?;
        let source=self.engine.debug_state_source()?;
        let mut candidate=self.engine.debugger_candidate(Probe::shared());candidate.functions.debug_probe=None;
        candidate.select_startup_imagemap(selected)?;
        let mut candidate=EngineStartup{engine:candidate,program:self.program.clone(),session:None,breakpoints:self.breakpoints.clone(),watches:self.watches.clone(),video_projection:self.video_projection.clone()};
        candidate.prepare_session()?;
        let mut batch=candidate.engine.build_scene_batch(candidate.engine.state.clone(),candidate.engine.runtime_epoch,candidate.engine.interface_epoch,candidate.engine.video_epoch)?;
        batch.display=candidate.engine.startup_imagemap_interaction()?;
        if batch.display.is_some(){return Err(self.engine.scene_resource_error("Selecting an Init hotspot cannot invent a second reached input header".into()));}
        self.engine.renderer.validate_scene_batch(&batch).map_err(|error|self.engine.scene_resource_error(error))?;
        if candidate.engine.debugger_anchor_measured()?.1>16*1024*1024-4096{return Err(self.engine.scene_resource_error("Init selection lacks bounded continuation headroom".into()));}
        let snapshot=candidate.snapshot();
        let progress=if candidate.is_complete(){DebugProgress::Complete}else{DebugProgress::Stopped};
        let video_projection=batch.videos.clone();
        Ok(Box::new(PreparedStartupStep{source,imagemap_source:Some(input.source.clone()),session_anchor:None,progress,batch:Some(batch),host_effects:StartupHostEffects::empty(),computation_state:None,video_projection,video_commands:Vec::new(),next_step_kind:None,terminal_completion:None,snapshot,session:candidate.session.take(),startup:candidate.engine.startup.clone(),
            persistent:candidate.engine.persistent_vars.clone(),input:candidate.engine.input_state.clone(),timer:candidate.engine.active_timer.clone(),host_random:candidate.engine.host_random.clone(),history:candidate.engine.history.debugger_copy(),locale:candidate.engine.locale.take()}))
    }
    pub fn begin_prepared_step_commit(&mut self,prepared:PreparedStartupStep)->Result<StartupStepCommitGuard<'_,R>,RuntimeError>{
        self.begin_prepared_step_commit_boxed(Box::new(prepared))
    }
    #[inline(never)]
    pub fn begin_prepared_step_commit_boxed(&mut self,prepared:Box<PreparedStartupStep>)->Result<StartupStepCommitGuard<'_,R>,RuntimeError>{
        self.validate_prepared_step(&prepared)?;
        Ok(StartupStepCommitGuard{owner:self,prepared})
    }
    pub fn validate_prepared_step(&self,prepared:&PreparedStartupStep)->Result<(),RuntimeError>{
        if !self.engine.accepts_debug_state_source(&prepared.source){return Err(self.engine.scene_resource_error("Startup Engine changed after step preparation".into()));}
        match (&self.session,prepared.session_anchor){
            (Some(session),Some(anchor)) if self.engine.accepts_debug_session(session)&&session.startup_step_anchor().map_err(|e|self.engine.scene_resource_error(e))?==anchor=>{},
            (None,None) if self.is_complete()&&prepared.imagemap_source.is_none()=>{},
            (None,None) if self.waiting_for_imagemap()&&prepared.imagemap_source.as_ref().is_some_and(|source|self.accepts_observation_source(source))=>{
                self.engine.validate_startup_imagemap_wait()?;
                self.engine.validate_startup_imagemap_gathers()?;
            },
            _=>return Err(self.engine.scene_resource_error("Startup continuation/configuration changed after preparation".into())),
        }
        if let Some(batch)=&prepared.batch{self.engine.renderer.validate_scene_batch(batch).map_err(|e|self.engine.scene_resource_error(e))?;}
        Ok(())
    }
    fn publish_step(&mut self,prepared:PreparedStartupStep)->DebugProgress{
        self.publish_step_boxed(Box::new(prepared))
    }
    #[inline(never)]
    fn publish_step_boxed(&mut self,prepared:Box<PreparedStartupStep>)->DebugProgress{
        let progress=prepared.progress;
        let(batch,effects)=self.install_step_boxed(prepared);
        if let Some(batch)=batch{self.engine.renderer.publish_scene_batch(batch);}
        effects.publish(&mut self.engine.renderer);
        progress
    }
    fn install_step(&mut self,prepared:PreparedStartupStep)->(Option<crate::renderer::SceneRenderBatch>,StartupHostEffects){
        self.install_step_boxed(Box::new(prepared))
    }
    #[inline(never)]
    fn install_step_boxed(&mut self,prepared:Box<PreparedStartupStep>)->(Option<crate::renderer::SceneRenderBatch>,StartupHostEffects){
        let PreparedStartupStep{progress,batch,host_effects,computation_state,video_projection,session,startup,persistent,input,timer,host_random,history,locale,..}=*prepared;
        self.video_projection=video_projection;
        if let Some(batch)=batch{
            self.engine.runtime_epoch=batch.runtime_epoch;self.engine.interface_epoch=batch.interface_epoch;self.engine.video_epoch=batch.video_epoch;
            self.engine.state=batch.state.clone();self.engine.startup=startup;self.engine.persistent_vars=persistent;self.engine.input_state=input;self.engine.active_timer=timer;self.engine.host_random=host_random;self.engine.history=history;self.engine.locale=locale;
            self.engine.interface_view_cache.lock().unwrap_or_else(|p|p.into_inner()).clear();
            self.session=session;
            return (Some(batch),host_effects);
        }
        if let Some(state)=computation_state{
            self.engine.state=state;self.engine.startup=startup;self.engine.persistent_vars=persistent;self.engine.input_state=input;self.engine.active_timer=timer;self.engine.host_random=host_random;self.engine.history=history;self.engine.locale=locale;
        }
        self.session=session;
        let _=progress;
        (None,host_effects)
    }
}
