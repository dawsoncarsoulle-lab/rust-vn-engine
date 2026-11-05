//! Read-only admission for the actual World driver; no evaluator is advanced.
use super::*;
impl<R:Renderer> EngineStartup<R>{
    pub fn driver_configuration(&self)->(Vec<wire::Breakpoint>,Vec<wire::Watch>){(self.breakpoints.clone(),self.watches.clone())}
    pub fn driver_armed_step_kind(&self)->Option<StepKind>{self.session.as_ref().and_then(DebugSession::armed_step_kind)}
    pub fn validate_driver_run(&self,kind:StepKind)->Result<(),RuntimeError>{
        if self.waiting_for_imagemap(){
            self.engine.validate_startup_imagemap_wait()?;
            if kind!=StepKind::Continue{return Err(self.engine.scene_resource_error("Executed Init input requires selection from its actual completed image; no statement can be stepped".into()));}
            return Ok(());
        }
        if self.waiting_for_video(){
            self.engine.validate_startup_video_wait()?;
            if kind!=StepKind::Continue{return Err(self.engine.scene_resource_error("Executed Init wait requires its actual media feedback; no statement can be stepped".into()));}
            return Ok(());
        }
        if self.is_complete()&&self.session.is_none(){
            return if kind==StepKind::Continue{Ok(())}else{Err(self.engine.scene_resource_error("Completed Init has no statement to step; its actual gameplay entry must finish".into()))};
        }
        let session=self.session.as_ref().ok_or_else(||self.engine.scene_resource_error("Startup is already complete".into()))?;
        if !self.engine.accepts_debug_session(session){return Err(self.engine.scene_resource_error("Actual startup continuation changed before Run admission".into()));}
        if session.armed_step_kind().is_some_and(|armed|armed!=kind){return Err(self.engine.scene_resource_error("A startup step is still running; resume the same command".into()));}
        if kind==StepKind::Out&&session.armed_step_kind().is_none()&&session.snapshot().frames.last().is_none_or(|frame|frame.depth==0){return Err(self.engine.scene_resource_error("No caller frame to step out to".into()));}
        Ok(())
    }
}
