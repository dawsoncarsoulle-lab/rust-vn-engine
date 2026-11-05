//! An executed Wait parks the actual authored Init cursor. It is not another
//! executable statement or a second evaluator playhead.
use super::*;

/// A pure capability observation, issued only for a real waiting Init owner.
/// Host focus/pause/loading and the original renderer gate remain necessary.
pub struct StartupVideoClockSource{
    source:StartupObservationSource,
    view:crate::video::VideoView,
}
impl<R:Renderer> Engine<R>{
    pub(in crate::engine) fn validate_startup_video_wait(&self)->Result<StartupOperation,RuntimeError>{
        self.validate_startup_video_wait_state(&self.state)?;
        self.startup_operation()
    }
    pub(in crate::engine) fn validate_startup_video_wait_state(&self,state:&GameState)->Result<(),RuntimeError>{
        let startup=self.startup.as_ref().ok_or_else(||self.scene_resource_error("Video wait has no actual pending Init".into()))?;
        let name=startup.waiting_video.as_ref().ok_or_else(||self.scene_resource_error("Initializer has no executed video wait".into()))?;
        let statement=startup.statement(&self.script).map_err(|error|self.scene_resource_error(error))?;
        if !matches!(Self::consumer_leaf(Some(statement)),Some(Statement::VideoWait{..}))
            ||state.videos.waiting.as_ref()!=Some(name)||startup.current().map(|root|root.pc)!=Some(state.pc){
            return Err(self.scene_resource_error("Executed video wait differs from its actual source/state".into()));
        }
        state.videos.validate().map_err(|error|self.scene_resource_error(error))?;
        if let Some(execution)=&state.scene_execution{
            let token=execution.resources.task(crate::scene_resources::ResourceTaskKind::Video,name,execution.owner())
                .and_then(|task|execution.resources.effect_token(self.runtime_epoch,&task.key,execution.owner()))
                .ok_or_else(||self.scene_resource_error("Executed Init wait has no actual owned video resource".into()))?;
            if !execution.resources.waits.iter().any(|wait|wait.owner==execution.owner()&&!wait.parked&&wait.key==token.task.key&&wait.revision==token.task.revision&&wait.target==token.target&&wait.position==execution.live_position){
                return Err(self.scene_resource_error("Executed Init wait lacks its exact resource continuation".into()));
            }
        }
        Ok(())
    }
    pub(in crate::engine) fn startup_video_wait_matches(&self,state:&GameState)->bool{self.validate_startup_video_wait_state(state).is_ok()}
}
impl<R:Renderer> EngineStartup<R>{
    pub(super) fn capture_waiting_save15(&self,slot:u32,label:String)->Result<crate::save::consumer_checkpoint::Save15,crate::save::SaveError>{
        if self.waiting_for_imagemap(){
            self.engine.validate_startup_imagemap_wait().map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;
            self.engine.validate_startup_imagemap_gathers().map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;
        }else{self.engine.validate_startup_video_wait().map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;}
        let save=self.engine.capture_startup_save15_inner(&self.program,None,slot,label,None)?;
        self.attach_waiting_save15(save)
    }
    /// The actual host holds its Native/PCM fences while supplying these exact
    /// snapshots. This never asks the renderer to capture the readers again.
    pub fn capture_save15_with_fenced_media(&self,slot:u32,label:String,snapshot:crate::audio_checkpoint::AudioCheckpointSnapshot,native:Vec<rvn_media::checkpoint::NativeCheckpoint>)->Result<crate::save::consumer_checkpoint::Save15,crate::save::SaveError>{
        if self.is_complete(){return Err(crate::save::SaveError::Incompatible("Completed startup must transfer its actual Engine before Save15 capture".into()));}
        self.validate_terminal_capture_phase()?;
        if self.waiting_for_video(){self.engine.validate_startup_video_wait().map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;}
        if self.waiting_for_imagemap(){self.engine.validate_startup_imagemap_wait().map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;self.engine.validate_startup_imagemap_gathers().map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;}
        let save=self.engine.capture_startup_save15_inner(&self.program,self.session.as_ref(),slot,label,Some((snapshot,native)))?;
        if self.waiting_for_video()||self.waiting_for_imagemap(){self.attach_waiting_save15(save)}else{Ok(save)}
    }
    fn attach_waiting_save15(&self,mut save:crate::save::consumer_checkpoint::Save15)->Result<crate::save::consumer_checkpoint::Save15,crate::save::SaveError>{
        use crate::save::{SaveError,consumer_checkpoint::{PortableDebugState,portable_namespaces}};
        let operation=if self.waiting_for_imagemap(){
            self.engine.validate_startup_imagemap_gathers().map_err(|error|SaveError::Incompatible(error.to_string()))?;
            self.engine.validate_startup_imagemap_wait()
        }else{self.engine.validate_startup_video_wait()}.map_err(|error|SaveError::Incompatible(error.to_string()))?;
        if self.session.is_some(){return Err(SaveError::Incompatible("Executed Init wait cannot carry a pending expression trace".into()));}
        let(namespaces,map)=portable_namespaces(&self.program,&self.engine.script,&self.engine.consumer_program)?;
        let mut startup=self.engine.portable_startup();
        if let Some(startup)=&mut startup{startup.remap_loop_namespaces(&map).map_err(SaveError::Incompatible)?;startup.remap_loop_origins(&self.program,true)?;}
        save.debugger=Some(PortableDebugState{startup,host_input:None,widget_materialization:None,frame_operation:None,trace:None,namespaces,
            pc:operation.pc,breakpoints:self.breakpoints.clone(),watches:self.watches.clone(),entry_stop:false,stopped:None,last_site:Some(operation.entry_site()),work:(0,0)});
        save.seal_enum_values1025()?;save.validate_enum_resources1025(&self.engine.functions.enum_assets1025)?;save.encode()?;Ok(save)
    }
    pub fn waiting_for_video(&self)->bool{
        self.session.is_none()&&self.engine.startup.as_ref().is_some_and(|startup|startup.waiting_video.is_some())
    }
    pub(super) fn wait_snapshot(&self)->wire::Snapshot{
        let operation=self.engine.validate_startup_video_wait().expect("Admitted executed Init wait retains its frozen source and state");
        let mut snapshot=self.engine.debug_current_snapshot(&self.watches).expect("Admitted bounded watches");
        snapshot.site=Some(operation.entry_site());snapshot.origin=Some(operation.origin);
        snapshot.evaluating=false;snapshot.running=false;snapshot.armed=None;
        snapshot.breakpoints=self.breakpoints.clone();snapshot
    }
    /// Reading this never evaluates UI, prepares a step, or opens an audio gate.
    pub fn video_clock_source(&self,view:&crate::video::VideoView)->Result<Option<StartupVideoClockSource>,RuntimeError>{
        if !self.waiting_for_video(){return Ok(None);}
        self.engine.validate_startup_video_wait()?;
        if self.session.is_some()||!self.video_projection.iter().any(|current|current==view)
            ||!self.engine.video_view_current(view)||view.track.playback!=crate::video::Playback::Playing{
            return Ok(None);
        }
        Ok(Some(StartupVideoClockSource{source:self.observation_source()?,view:view.clone()}))
    }
    pub fn accepts_video_clock_source(&self,source:&StartupVideoClockSource)->bool{
        self.accepts_observation_source(&source.source)&&self.waiting_for_video()
            &&self.session.is_none()&&self.engine.validate_startup_video_wait().is_ok()
            &&self.video_projection.iter().any(|current|current==&source.view)
            &&self.engine.video_view_current(&source.view)
            &&source.view.track.playback==crate::video::Playback::Playing
    }
}
