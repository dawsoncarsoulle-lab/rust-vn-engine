//! Startup keeps the existing queue's unique nonce identities until true ack.
use super::*;
struct MetadataCost{bytes:usize}
impl std::fmt::Write for MetadataCost{
    fn write_str(&mut self,value:&str)->std::fmt::Result{
        self.bytes=self.bytes.checked_add(value.len()).filter(|bytes|*bytes<=rvn_core::engine::MAX_STARTUP_VIDEO_METADATA_BYTES).ok_or(std::fmt::Error)?;Ok(())
    }
}
fn position_cost(view:&rvn_core::video::VideoView,feedback:&VideoFeedback)->Result<usize,String>{
    use std::fmt::Write;
    let VideoFeedback::Report(rvn_core::video::Feedback::Position{seconds,duration})=feedback else{return Ok(0);};
    let mut cost=MetadataCost{bytes:0};write!(&mut cost,"{view:?}{seconds:?}{duration:?}").map_err(|_|"Actual Startup position metadata exceeds the existing Core64KiB contract before cloning".to_string())?;Ok(cost.bytes)
}
pub(crate) struct StartupTerminalTicket{snapshot:VideoQueueSnapshot,ordinal:usize}
fn feedback_equal(a:&rvn_core::GameVideoFeedback,b:&VideoFeedback)->bool{
    use rvn_core::{GameVideoFeedback as G,video::Feedback as F};
    match(a,b){(G::Resume,VideoFeedback::Resume)=>true,(G::Report(F::End),VideoFeedback::Report(F::End))=>true,
        (G::Report(F::Error(a)),VideoFeedback::Report(F::Error(b)))|(G::Report(F::AutoplayBlocked(a)),VideoFeedback::Report(F::AutoplayBlocked(b)))=>a==b,_=>false}
}
pub(crate) fn acknowledge_startup_terminal(world:&mut World,ticket:StartupTerminalTicket,receipt:&rvn_core::engine::AcceptedStartupVideoTerminal)->Result<(),String>{
    let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>().ok_or("Actual terminal owner vanished before acknowledgment")?;
    if startup.publication_pending||!startup.owner.accepts_video_terminal_receipt(receipt){return Err("Actual terminal publication/source has not been installed".into());}
    let row=ticket.snapshot.rows.get(ticket.ordinal).ok_or("Invalid actual terminal ordinal")?;
    if receipt.prior_view()!=&row.0||!feedback_equal(receipt.feedback(),&row.1){return Err("Completed callback does not belong to the retained actual queue nonce".into());}
    world.resource_mut::<VideoFeedbackQueue>().acknowledge(&ticket.snapshot,&[rvn_core::VideoReportReceipt{ordinal:ticket.ordinal,disposition:rvn_core::VideoReportDisposition::Consumed}])
}
pub(crate) fn startup_video_feedback(world:&mut World)->Result<(),String>{
    let actual=world.resource::<crate::debugger_startup::DebugBootstrap>();
    if actual.failed.is_some()||actual.publication_pending||actual.mode!=Some(rvn_core::editor_debugger::StepKind::Continue)||crate::editor_preview::paused(world)||crate::checkpoint_requests::world_blocked(world)||crate::debugger_startup::driver::pending(world)||!actual.owner.waiting_for_video()||crate::debugger_startup::driver::terminal_pending(world){return Ok(());}
    let snapshot=world.resource::<VideoFeedbackQueue>().snapshot()?;if snapshot.rows.is_empty(){return Ok(());}
    let current=actual.owner.video_views().map_err(|error|error.to_string())?;
    let index:std::collections::BTreeMap<_,_>=current.iter().map(|view|(view.id.as_str(),view)).collect();
    let mut selected=VideoQueueSnapshot{rows:Vec::new(),nonces:Vec::new()};let mut metadata=0usize;
    // Identity is resolved from the genuine owner before reading the payload.
    // This index only selects work; Core admission remains obligatory below.
    for ((view,feedback),nonce) in snapshot.rows.iter().zip(&snapshot.nonces){
        let member=index.get(view.id.as_str()).filter(|actual|***actual==*view);
        let eligible=member.is_none()||member.is_some_and(|actual|actual.track.playback==rvn_core::video::Playback::Playing&&!actual.track.finished())&&matches!(feedback,VideoFeedback::Report(rvn_core::video::Feedback::Position{..}));
        if eligible{
            // Stale rejection examines no payload. Current positions use the
            // same bounded streaming Debug cost as the genuine Core preflight.
            let cost=if member.is_some(){position_cost(view,feedback)?}else{0};
            if metadata.checked_add(cost).is_none_or(|bytes|bytes>rvn_core::engine::MAX_STARTUP_VIDEO_METADATA_BYTES){break;}
            metadata+=cost;selected.rows.push((view.clone(),feedback.clone()));selected.nonces.push(*nonce);if selected.rows.len()==rvn_core::engine::MAX_STARTUP_VIDEO_REPORTS{break;}
        }
    }
    if !selected.rows.is_empty(){
        let reports:Vec<_>=selected.rows.iter().map(|(view,feedback)|rvn_core::engine::StartupVideoReport{view:view.clone(),feedback:match feedback{VideoFeedback::Report(feedback)=>rvn_core::GameVideoFeedback::Report(feedback.clone()),VideoFeedback::Resume=>rvn_core::GameVideoFeedback::Resume}}).collect();
        let prepared=actual.owner.prepare_video_feedback(&reports).map_err(|error|error.to_string())?;
        #[cfg(all(feature="video",not(target_arch="wasm32")))]
        let native=crate::native_video::prepare_positions(world,&prepared)?;
        let committed=world.resource_mut::<crate::debugger_startup::DebugBootstrap>().owner.begin_prepared_video_feedback_commit(prepared).map_err(|error|error.to_string())?.commit();
        #[cfg(all(feature="video",not(target_arch="wasm32")))]
        crate::native_video::install_positions(world,native,&committed);
        {let mut queue=world.resource_mut::<VideoFeedbackQueue>();for row in &mut queue.0{for receipt in committed.accepted_reports(){if row.0==*receipt.prior_view(){row.0=receipt.accepted_view().clone();break;}}}queue.acknowledge(&selected,committed.receipts())?;}
    }
    let snapshot=world.resource::<VideoFeedbackQueue>().snapshot()?;
    let current=world.resource::<crate::debugger_startup::DebugBootstrap>().owner.video_views().map_err(|error|error.to_string())?;
    let index:std::collections::BTreeMap<_,_>=current.iter().map(|view|(view.id.as_str(),view)).collect();
    // At most128 reports are committed in this frame. Capture all applicable
    // Position chunks before starting a terminal; Deferred blocked positions
    // must not prevent the real Resume callback from executing.
    if snapshot.rows.iter().any(|(view,feedback)|index.get(view.id.as_str()).is_some_and(|actual|**actual==*view&&actual.track.playback==rvn_core::video::Playback::Playing&&!actual.track.finished())&&matches!(feedback,VideoFeedback::Report(rvn_core::video::Feedback::Position{..}))){return Ok(());}
    for(ordinal,(view,feedback))in snapshot.rows.iter().enumerate(){
        if !index.get(view.id.as_str()).is_some_and(|actual|**actual==*view)||matches!(feedback,VideoFeedback::Report(rvn_core::video::Feedback::Position{..})){continue;}
        let report=rvn_core::engine::StartupVideoReport{view:view.clone(),feedback:match feedback{VideoFeedback::Report(feedback)=>rvn_core::GameVideoFeedback::Report(feedback.clone()),VideoFeedback::Resume=>rvn_core::GameVideoFeedback::Resume}};
        let prepared=world.resource::<crate::debugger_startup::DebugBootstrap>().owner.prepare_video_continuation(&report).map_err(|error|error.to_string())?;
        world.resource_mut::<crate::debugger_startup::DebugBootstrap>().owner.begin_prepared_video_continuation(prepared).map_err(|error|error.to_string())?;
        crate::debugger_startup::driver::retain_terminal(world,StartupTerminalTicket{snapshot,ordinal});break;
    }
    Ok(())
}
