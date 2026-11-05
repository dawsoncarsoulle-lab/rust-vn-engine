//! Accepted Position reports are computation, never a renderer/decoder restart.
//! Terminal callbacks remain queued until their genuine debugger operation runs.
use super::*;
use std::sync::Arc;
pub const MAX_STARTUP_VIDEO_REPORTS:usize=128;
pub const MAX_STARTUP_VIDEO_METADATA_BYTES:usize=64*1024;
/// Data submitted by the existing host queue. It grants no owner permission.
pub struct StartupVideoReport{pub view:crate::video::VideoView,pub feedback:GameVideoFeedback}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum StartupVideoReportKind{Position}
struct AcceptedPosition{ordinal:usize,prior:crate::video::VideoView,accepted:crate::video::VideoView}
/// No constructor/serde; one exact actual waiting owner prepares this candidate.
pub struct PreparedStartupVideoFeedback{
    source:StartupObservationSource,next:crate::video::VideoState,projection:Vec<crate::video::VideoView>,
    post:Option<Arc<StartupObservationSource>>,receipts:Vec<VideoReportReceipt>,accepted:Vec<AcceptedPosition>,
}
impl PreparedStartupVideoFeedback{
    /// Data for the real host's preflight, never a post-commit receipt.
    pub fn position_views(&self)->impl Iterator<Item=(&crate::video::VideoView,&crate::video::VideoView)> {
        self.accepted.iter().map(|row|(&row.prior,&row.accepted))
    }
}
pub struct StartupVideoFeedbackGuard<'a,R:Renderer>{owner:&'a mut EngineStartup<R>,prepared:PreparedStartupVideoFeedback}
/// A receipt exists only after the exact Core source/state was committed.
/// Its private post-source must still match the current owner before Native ack.
#[derive(Clone)]
pub struct AcceptedStartupVideoReport{post:Arc<StartupObservationSource>,ordinal:usize,prior:crate::video::VideoView,accepted:crate::video::VideoView}
impl AcceptedStartupVideoReport{
    pub fn ordinal(&self)->usize{self.ordinal}
    pub fn kind(&self)->StartupVideoReportKind{StartupVideoReportKind::Position}
    pub fn prior_view(&self)->&crate::video::VideoView{&self.prior}
    pub fn accepted_view(&self)->&crate::video::VideoView{&self.accepted}
}
pub struct CommittedStartupVideoFeedback{receipts:Vec<VideoReportReceipt>,accepted:Vec<AcceptedStartupVideoReport>}
impl CommittedStartupVideoFeedback{
    pub fn receipts(&self)->&[VideoReportReceipt]{&self.receipts}
    pub fn accepted_reports(&self)->&[AcceptedStartupVideoReport]{&self.accepted}
}
impl<R:Renderer> StartupVideoFeedbackGuard<'_,R>{
    pub fn commit(self)->CommittedStartupVideoFeedback{
        let PreparedStartupVideoFeedback{next,projection,post,receipts,accepted,..}=self.prepared;
        // No callback, renderer, epoch, waiter or evaluator can change here.
        self.owner.engine.state.videos=next;self.owner.video_projection=projection;
        let accepted=accepted.into_iter().map(|row|AcceptedStartupVideoReport{post:post.as_ref().expect("Accepted positions retain their measured post-source").clone(),ordinal:row.ordinal,prior:row.prior,accepted:row.accepted}).collect();
        CommittedStartupVideoFeedback{receipts,accepted}
    }
}
impl<R:Renderer> EngineStartup<R>{
    pub fn prepare_video_feedback(&self,reports:&[StartupVideoReport])->Result<PreparedStartupVideoFeedback,RuntimeError>{
        if reports.len()>MAX_STARTUP_VIDEO_REPORTS{return Err(self.engine.scene_resource_error("Startup video report count exceeds 128".into()));}
        self.engine.validate_startup_video_wait()?;
        if self.session.is_some(){return Err(self.engine.scene_resource_error("Actual Init callback/evaluator must settle before another media report".into()));}
        let source=self.observation_source()?;
        let mut positions=Vec::new();let mut receipts=Vec::with_capacity(reports.len());let mut bytes=Fingerprint{hash:Sha256::new(),bytes:0};
        for(ordinal,report)in reports.iter().enumerate(){
            // Resolve the actual source member before examining its payload.
            let current=self.video_projection.iter().find(|view|*view==&report.view).filter(|view|self.engine.video_view_current(view));
            let disposition=if let Some(view)=current{
                if let GameVideoFeedback::Report(crate::video::Feedback::Position{seconds,duration})=&report.feedback{
                    if view.track.playback!=crate::video::Playback::Playing||view.track.finished(){VideoReportDisposition::Deferred}else{
                        write!(&mut bytes,"{view:?}{seconds:?}{duration:?}").map_err(|_|self.engine.scene_resource_error("Startup video report metadata exceeds its bounded source".into()))?;
                        if bytes.bytes>MAX_STARTUP_VIDEO_METADATA_BYTES{return Err(self.engine.scene_resource_error("Startup video report metadata exceeds 64 KiB".into()));}
                        positions.push((ordinal,view,*seconds,*duration));VideoReportDisposition::Consumed
                    }
                }else{VideoReportDisposition::Deferred}
            }else{VideoReportDisposition::RejectedStale};
            receipts.push(VideoReportReceipt{ordinal,disposition});
        }
        let mut candidate=self.engine.debugger_candidate(Probe::shared());candidate.functions.debug_probe=None;
        for(_,view,seconds,duration)in &positions{
            if !candidate.video_feedback_from_view(view,crate::video::Feedback::Position{seconds:*seconds,duration:*duration})?{return Err(self.engine.scene_resource_error("Actual admitted Init position lost its resource source".into()));}
        }
        candidate.validate_startup_video_wait()?;
        let mut projection=self.video_projection.clone();
        for view in &mut projection{view.track=candidate.state.videos.tracks.get(&view.id).ok_or_else(||self.engine.scene_resource_error("Actual Init position lost its accepted projection".into()))?.clone();view.subtitle=view.track.clip.subtitle(view.track.position).map(|cue|candidate.translate(&cue.text).to_string());}
        if candidate.debugger_anchor_measured()?.1>16*1024*1024-4096{return Err(self.engine.scene_resource_error("Startup feedback anchor lacks bounded headroom".into()));}
        let next=candidate.state.videos.clone();
        let candidate=EngineStartup{engine:candidate,program:self.program.clone(),session:None,breakpoints:self.breakpoints.clone(),watches:self.watches.clone(),video_projection:projection.clone()};
        let post=if positions.is_empty(){None}else{Some(Arc::new(candidate.observation_source()?))};
        let accepted=positions.into_iter().map(|(ordinal,prior,_,_)|AcceptedPosition{ordinal,prior:prior.clone(),accepted:projection.iter().find(|view|view.id==prior.id).expect("Validated accepted member").clone()}).collect();
        Ok(PreparedStartupVideoFeedback{source,next,projection,post,receipts,accepted})
    }
    pub fn validate_prepared_video_feedback(&self,prepared:&PreparedStartupVideoFeedback)->Result<(),RuntimeError>{
        if self.session.is_some()||!self.accepts_observation_source(&prepared.source){return Err(self.engine.scene_resource_error("Startup video source/configuration changed after report preparation".into()));}
        self.engine.validate_startup_video_wait()?;Ok(())
    }
    pub fn begin_prepared_video_feedback_commit(&mut self,prepared:PreparedStartupVideoFeedback)->Result<StartupVideoFeedbackGuard<'_,R>,RuntimeError>{self.validate_prepared_video_feedback(&prepared)?;Ok(StartupVideoFeedbackGuard{owner:self,prepared})}
    pub fn accepts_video_report_receipt(&self,receipt:&AcceptedStartupVideoReport)->bool{
        self.accepts_observation_source(&receipt.post)&&self.video_projection.iter().any(|view|view==&receipt.accepted)&&self.engine.video_view_current(&receipt.accepted)
    }
}
