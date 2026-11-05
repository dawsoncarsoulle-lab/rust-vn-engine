//! Actual media callbacks reuse the admitted Init command and true evaluator.
//! The authored Wait remains the entry site; no synthetic Statement or PC.
use super::*;
#[derive(Clone,Debug)]
pub(super) struct AdmittedStartupVideoFeedback{view:crate::video::VideoView,feedback:GameVideoFeedback}
/// Created only when the real callback operation has completed, then carried
/// through its actual host commit. It authorizes no client-created operation.
#[derive(Clone)]
pub struct AcceptedStartupVideoTerminal{
    pub(super) post:std::sync::Arc<StartupObservationSource>,
    prior:crate::video::VideoView,feedback:GameVideoFeedback,
}
impl AcceptedStartupVideoTerminal{
    pub fn prior_view(&self)->&crate::video::VideoView{&self.prior}
    pub fn feedback(&self)->&GameVideoFeedback{&self.feedback}
}
impl<R:Renderer> EngineStartup<R>{
    pub fn accepts_video_terminal_receipt(&self,receipt:&AcceptedStartupVideoTerminal)->bool{
        self.accepts_observation_source(&receipt.post)
    }
    pub(super) fn completed_terminal_receipt<T:Renderer>(&self,before:&EngineStartup<T>)->Result<Option<AcceptedStartupVideoTerminal>,RuntimeError>{
        let Some(DebugSession{operation:DebugOperation::Startup(operation),..})=before.session.as_ref()else{return Ok(None);};
        let Some(report)=&operation.video_feedback else{return Ok(None);};
        // An interior breakpoint still owns the same admitted operation. Only
        // prepare_session after its actual completion removes that operation.
        if self.session.as_ref().is_some_and(|session|matches!(&session.operation,DebugOperation::Startup(operation)if operation.video_feedback.is_some())){return Ok(None);}
        Ok(Some(AcceptedStartupVideoTerminal{post:std::sync::Arc::new(self.observation_source()?),prior:report.view.clone(),feedback:report.feedback.clone()}))
    }
}
/// Installing this opaque candidate starts a real callback debugger session;
/// it does not acknowledge the queue or publish any renderer/media effect.
pub struct PreparedStartupVideoContinuation{source:StartupObservationSource,session:DebugSession}
impl<R:Renderer> EngineStartup<R>{
    pub fn prepare_video_continuation(&self,report:&StartupVideoReport)->Result<PreparedStartupVideoContinuation,RuntimeError>{
        if !self.waiting_for_video(){return Err(self.engine.scene_resource_error("Video continuation requires the actual executed Init wait".into()));}
        self.engine.validate_startup_video_wait()?;
        let source=self.observation_source()?;
        let view=self.video_projection.iter().find(|view|*view==&report.view).filter(|view|self.engine.video_view_current(view)).ok_or_else(||self.engine.scene_resource_error("Startup video continuation belongs to a stale actual member".into()))?;
        let track=&view.track;
        let accepted=match &report.feedback{
            GameVideoFeedback::Report(crate::video::Feedback::Position{..})=>false,
            GameVideoFeedback::Report(crate::video::Feedback::End)=>track.playback==crate::video::Playback::Playing&&!track.finished(),
            GameVideoFeedback::Report(crate::video::Feedback::Error(_)|crate::video::Feedback::AutoplayBlocked(_))=>!track.finished(),
            GameVideoFeedback::Resume=>track.playback==crate::video::Playback::Blocked,
        };
        if !accepted{return Err(self.engine.scene_resource_error("Video continuation is not executable in this actual playback phase".into()));}
        let mut bytes=Fingerprint{hash:Sha256::new(),bytes:0};write!(&mut bytes,"{view:?}{:?}",report.feedback).map_err(|_|self.engine.scene_resource_error("Startup terminal metadata exceeds bounded source".into()))?;
        if bytes.bytes>64*1024{return Err(self.engine.scene_resource_error("Startup terminal metadata exceeds64 KiB before cloning".into()));}
        let mut operation=self.engine.startup_operation()?;
        operation.video_feedback=Some(AdmittedStartupVideoFeedback{view:view.clone(),feedback:report.feedback.clone()});
        let mut session=self.engine.debug_begin(DebugOperation::Startup(operation),self.breakpoints.clone())?;
        session.set_watches(self.watches.clone()).map_err(|error|self.engine.scene_resource_error(error))?;
        Ok(PreparedStartupVideoContinuation{source,session})
    }
    pub fn validate_prepared_video_continuation(&self,prepared:&PreparedStartupVideoContinuation)->Result<(),RuntimeError>{
        if !self.waiting_for_video()||!self.accepts_observation_source(&prepared.source)||!self.engine.accepts_debug_session(&prepared.session){return Err(self.engine.scene_resource_error("Actual Startup source/session changed before video continuation admission".into()));}
        self.engine.validate_startup_video_wait()?;Ok(())
    }
    pub fn begin_prepared_video_continuation(&mut self,prepared:PreparedStartupVideoContinuation)->Result<(),RuntimeError>{
        self.validate_prepared_video_continuation(&prepared)?;self.session=Some(prepared.session);Ok(())
    }
    pub(super) fn validate_terminal_capture_phase(&self)->Result<(),crate::save::SaveError>{
        if let Some(session)=&self.session{if let DebugOperation::Startup(operation)=&session.operation{self.engine.portable_startup_for_operation(operation).map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;}}Ok(())
    }
}
impl<R:Renderer> Engine<R>{
    pub(super) fn execute_startup_video_feedback(&mut self,operation:StartupOperation)->Result<(),RuntimeError>{
        self.validate_startup_operation(&operation)?;self.validate_startup_video_wait()?;
        let report=operation.video_feedback.as_ref().ok_or_else(||self.scene_resource_error("Startup media operation has no admitted actual intent".into()))?;
        if !self.video_view_current(&report.view){return Err(self.scene_resource_error("Startup media operation lost its actual owned view".into()));}
        let probe=self.functions.debug_probe.clone().ok_or_else(||self.scene_resource_error("Startup callback requires its actual debugger continuation".into()))?;
        let mut candidate=self.debugger_candidate(probe.clone());
        let accepted=Probe::initializer_route(&probe,operation.route.clone(),||match &report.feedback{
            GameVideoFeedback::Report(feedback)=>candidate.video_feedback_from_view(&report.view,feedback.clone()),
            GameVideoFeedback::Resume=>candidate.resume_video_from_view(&report.view),
        }).map_err(|error|self.eval_err(error,"Actual initializer media callback"))??;
        if !accepted{return Err(self.scene_resource_error("Admitted Startup callback was not consumed by its actual member".into()));}
        if candidate.state.videos.waiting.is_none(){
            let startup=candidate.startup.as_mut().ok_or_else(||self.scene_resource_error("Actual callback lost its Init cursor".into()))?;startup.waiting_video=None;startup.advance(&candidate.script).map_err(|error|self.scene_resource_error(error))?;
            candidate.state.pc=startup.current().map_or(startup.exit_pc,|root|root.pc);if startup.current().is_none(){candidate.startup=None;}candidate.sync_narrative_position();
            crate::scene_resources::sync_current(&mut candidate.state).map_err(|error|self.scene_resource_error(error))?;
        }
        let mut batch=candidate.build_scene_batch(candidate.state.clone(),candidate.runtime_epoch,candidate.interface_epoch,candidate.video_epoch)?;batch.previous=self.state.clone();self.renderer.validate_scene_batch(&batch).map_err(|error|self.scene_resource_error(error))?;
        let(startup,persistent,input,timer,host_random,history,locale)=(candidate.startup.clone(),candidate.persistent_vars.clone(),candidate.input_state.clone(),candidate.active_timer.clone(),candidate.host_random.clone(),candidate.history.debugger_copy(),candidate.locale.take());drop(candidate);
        self.state=batch.state.clone();self.runtime_epoch=batch.runtime_epoch;self.interface_epoch=batch.interface_epoch;self.video_epoch=batch.video_epoch;self.startup=startup;self.persistent_vars=persistent;self.input_state=input;self.active_timer=timer;self.host_random=host_random;self.history=history;self.locale=locale;self.interface_view_cache.lock().unwrap_or_else(|p|p.into_inner()).clear();self.renderer.publish_scene_batch(batch);Ok(())
    }
}

/// Typed saved media intent. Source roots/state/projection are reconstructed by
/// the actual Engine; this contains no transient view,token,epoch or AST.
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PortableStartupVideoCallback{pub(super) waiting_name:String,name:String,payload:crate::engine::frame_continuation::VideoPayload}
impl PortableStartupVideoCallback{
    pub(super) fn validate(&self)->Result<(),crate::save::SaveError>{
        for name in [&self.name,&self.waiting_name]{if name.len()>128||!rvn_ui::composition::valid_name(name){return Err(crate::save::SaveError::Incompatible("Saved actual startup media identity exceeds its bounds".into()));}}
        if matches!(self.payload,crate::engine::frame_continuation::VideoPayload::Position{..}){return Err(crate::save::SaveError::Incompatible("Position is computation,not a pending startup callback".into()));}
        self.payload.restore()?;Ok(())
    }
}
impl<R:Renderer> Engine<R>{
    pub(in crate::engine) fn portable_startup_for_operation(&self,operation:&StartupOperation)->Result<Option<PortableStartup>,RuntimeError>{
        self.validate_startup_operation(operation)?;
        let mut startup=self.portable_startup().ok_or_else(||self.scene_resource_error("Actual startup operation has no portable source cursor".into()))?;
        if let Some(report)=&operation.video_feedback{
            self.validate_startup_video_wait()?;
            if !self.video_view_current(&report.view){return Err(self.scene_resource_error("Saved startup callback lost its actual source member".into()));}
            let callback=PortableStartupVideoCallback{waiting_name:self.startup.as_ref().and_then(|startup|startup.waiting_video.clone()).ok_or_else(||self.scene_resource_error("Actual callback has no executed Wait".into()))?,name:report.view.id.clone(),payload:crate::engine::frame_continuation::VideoPayload::capture(&report.feedback)};
            callback.validate().map_err(|error|self.scene_resource_error(error.to_string()))?;startup.waiting=None;startup.callback=Some(callback);
        }Ok(Some(startup))
    }
    pub(in crate::engine) fn restore_startup_video_operation(&self,startup:&PortableStartup,actual_views:&[crate::video::VideoView])->Result<StartupOperation,RuntimeError>{
        startup.validate_callback_wire().map_err(|error|self.scene_resource_error(error.to_string()))?;
        let mut operation=self.startup_operation()?;
        if let Some(callback)=&startup.callback{
            self.validate_startup_video_wait()?;
            let view=actual_views.iter().find(|view|view.id==callback.name&&self.video_view_current(view)).ok_or_else(||self.scene_resource_error("Saved startup callback has no genuine restored video member".into()))?;
            let feedback=callback.payload.restore().map_err(|error|self.scene_resource_error(error.to_string()))?;
            let active=match &feedback{GameVideoFeedback::Report(crate::video::Feedback::End)=>view.track.playback==crate::video::Playback::Playing&&!view.track.finished(),GameVideoFeedback::Resume=>view.track.playback==crate::video::Playback::Blocked,GameVideoFeedback::Report(crate::video::Feedback::Error(_)|crate::video::Feedback::AutoplayBlocked(_))=>!view.track.finished(),_=>false};
            if !active{return Err(self.scene_resource_error("Saved startup callback contradicts actual playback phase".into()));}
            operation.video_feedback=Some(AdmittedStartupVideoFeedback{view:view.clone(),feedback});
        }Ok(operation)
    }
}

/// A real media operation owns this borrow. Resident UI uses its exact target;
/// story/legacy media retain their own source owner, never a fake Host instance.
pub(in crate::engine) struct StartupMediaScopePermit<'a,R:Renderer>{engine:&'a Engine<R>,operation:&'a StartupOperation,allowed:std::collections::BTreeSet<String>,namespaces:std::collections::BTreeMap<String,std::collections::BTreeMap<String,String>>}
impl<R:Renderer> crate::ui_consumer::PortableScopePermit for StartupMediaScopePermit<'_,R>{
    fn admits(&self,scope:&str)->bool{self.allowed.contains(scope)&&self.engine.validate_startup_operation(self.operation).is_ok()&&self.engine.validate_startup_media_owner(self.operation).is_ok()}
}
impl<R:Renderer> StartupMediaScopePermit<'_,R>{pub(in crate::engine) fn namespaces(&self)->&std::collections::BTreeMap<String,std::collections::BTreeMap<String,String>>{&self.namespaces}}
impl<R:Renderer> Engine<R>{
    fn validate_startup_media_owner(&self,operation:&StartupOperation)->Result<(),RuntimeError>{
        self.validate_startup_video_wait()?;let report=operation.video_feedback.as_ref().ok_or_else(||self.scene_resource_error("Media scope requires the actual admitted callback".into()))?;
        if !self.video_view_current(&report.view){return Err(self.scene_resource_error("Media scope lost its exact current video capability".into()));}
        if let Some(binding)=&report.view.ui_projection{
            crate::ui_residency::resident_interface(&self.state,&binding.interface).map_err(|error|self.scene_resource_error(error))?;
        }else if report.view.track.clip.target.is_some()||report.view.resource_token.as_ref().is_some_and(|token|matches!(token.task.key.owner,crate::scene_tasks::TaskOwner::Host{..})){
            return Err(self.scene_resource_error("Media scope cannot replace a missing actual UI/Host target with Game".into()));
        }else if report.view.resource_token.is_none()&&self.narrative.is_some(){return Err(self.scene_resource_error("Narrative media scope has no actual resource token".into()));}
        Ok(())
    }
    pub(in crate::engine) fn startup_media_scope_permit<'a>(&'a self,program:&rvn_parser::ParsedProgram,operation:&'a DebugOperation)->Result<Option<StartupMediaScopePermit<'a,R>>,crate::save::SaveError>{
        use crate::save::SaveError;
        let DebugOperation::Startup(operation)=operation else{return Ok(None);};let Some(report)=&operation.video_feedback else{return Ok(None);};
        self.validate_save15_program(program)?;self.validate_startup_operation(operation).map_err(|error|SaveError::Incompatible(error.to_string()))?;self.validate_startup_media_owner(operation).map_err(|error|SaveError::Incompatible(error.to_string()))?;
        let handler=match &report.feedback{GameVideoFeedback::Report(crate::video::Feedback::End)=>report.view.track.clip.on_end.as_ref(),GameVideoFeedback::Report(crate::video::Feedback::Error(_))=>report.view.track.clip.on_error.as_ref(),_=>None};
        let Some(handler)=handler else{return Ok(None);};
        let binding=crate::consumer_schedule::ConsumerSourceBinding::from_program(program).map_err(|error|SaveError::Incompatible(error.to_string()))?;
        binding.definition(&crate::consumer_schedule::HostDefinitionKind::Handler,handler).ok_or_else(||SaveError::Incompatible("Actual media handler has no frozen source definition/ordinal".into()))?;
        let mut allowed=std::collections::BTreeSet::new();let mut namespaces=std::collections::BTreeMap::new();
        for(root,definition)in &binding.definitions{
            allowed.insert(root.clone());let scopes=definition.scopes.keys().map(|scope|(scope.clone(),format!("startup-media-source:{}:{}:{}:{}",definition.source_ordinal,root.len(),root,scope))).collect();namespaces.insert(root.clone(),scopes);
        }
        Ok(Some(StartupMediaScopePermit{engine:self,operation,allowed,namespaces}))
    }
}
