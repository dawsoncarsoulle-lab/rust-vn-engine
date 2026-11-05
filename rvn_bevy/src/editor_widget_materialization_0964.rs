//! One existing debugger mailbox/VM owns the resumed saved transaction and
//! the later new wrapper life. Physical capture/paint remains explicitly gated.
use super::*;
fn finish_publication(_world:&mut World,_outcome:DebugOutcome)->Result<(),String>{
 // This First driver precedes the one ordinary ScenePublication consumer.
 // The owned VM already committed its batch; leave that exact batch/output to
 // its actual Source/World fence, rather than consume or acknowledge a second
 // queue here. Restored logical reports have no new external report receipt.
 Ok(())
}
/// Transfer, never copy, the caller's actual returned Save15 continuation into
/// the existing driver. Failure preserves its ownership in `session`.
pub(crate) fn adopt_restored_widget_session(world:&mut World,session:&mut Option<DebugSession>,mode:Option<StepKind>)->Result<(),String>{
 let actual=session.as_ref().ok_or("No actual restored session to adopt")?;
 let game=world.get_resource::<VnEngine>().ok_or("Restored Engine is missing")?;
 if !game.0.accepts_debug_session(actual)||actual.is_complete(){return Err("Restored session differs from the actual logical owner/source".into());}
 if matches!(actual.operation(),DebugOperation::Startup(_)){return Err("Pending Init belongs to its actual EngineStartup driver".into());}
 let operation=actual.operation().clone();let story=matches!(operation,DebugOperation::Statement);
 let(breakpoints,watches)=actual.configuration();
 let mut next=Mailbox::default();next.enabled=true;next.breakpoints=breakpoints.to_vec();next.watches=watches.to_vec();next.mode=mode;
 next.snapshot=Some(game.0.debug_session_snapshot(actual));
 let expected=match &operation{DebugOperation::Input(_)=>1,DebugOperation::GameFrame{..}=>2,DebugOperation::HostFrame{..}=>3,DebugOperation::RestoredFrame(_)=>4,DebugOperation::MaterializeWidget(_)=>5,_=>0};
 let finish:Option<Finish>=if story{None}else{Some(Box::new(move|world,outcome|{
  if !matches!((expected,&outcome),(1,DebugOutcome::Input(_))|(2,DebugOutcome::Game(_))|(3,DebugOutcome::Host(_))|(4,DebugOutcome::Game(_))|(4,DebugOutcome::Host(_))|(5,DebugOutcome::WidgetMaterialized)){return Err("Restored owned operation returned a different receipt kind".into());}
  finish_publication(world,outcome)
 }))};
 next.admit(game,operation,finish,0)?;next.story_queued=story;
 let identity=(game.0.instance_id(),game.0.runtime_epoch());let shared=game.0.renderer.debugger.clone();
 let mut mailbox=shared.lock().map_err(|_|"Actual debugger mailbox is unavailable")?;
 if mailbox.active.as_ref().is_some_and(|active|(active.queued.instance,active.queued.epoch)==identity)||mailbox.queue.iter().any(|queued|(queued.instance,queued.epoch)==identity){return Err("Another current owner already holds the real debugger mailbox".into());}
 // Every fallible admission finished before transferring the sole session.
 let owned=session.take().expect("validated actual session remains owned");
 let queued=next.queue.pop_front().expect("bounded admission queued its exact operation");
 next.active=Some(Active{queued,session:owned,restore_pause:mode!=Some(StepKind::Continue),needs_pause:true});
 let mut stale=std::mem::replace(&mut *mailbox,next);drop(mailbox);
 let mut discarded=std::mem::take(&mut stale.queue);if let Some(active)=stale.active.take(){discarded.push_front(active.queued);}
 for mut queued in discarded{if let Some(abort)=queued.abort.take(){abort(world);}}
 Ok(())
}
/// Existing First boundary, before fresh clocks/input and ScenePublication.
/// Preview registers it after control and before its one debugger drain; normal
/// runtime registers it once and uses the same drain only if that owner exists.
pub(crate) fn drive_widget_materialization(world:&mut World){
 if world.contains_resource::<crate::debugger_startup::DebugBootstrap>(){return;}
 if world.get_resource::<State<VnState>>().is_some_and(|state|*state.get()==VnState::Error){return;}
 if world.get_resource::<crate::checkpoint_requests::CheckpointRequests>().is_some_and(|requests|requests.blocked()){return;}
 let Some(game)=world.get_resource::<VnEngine>()else{return;};
 if game.0.renderer.scene_batch_pending(){return;}
 let shared=game.0.renderer.debugger.clone();let normal=world.get_resource::<crate::editor_preview::Output>().is_none();
 let(enabled,active,failed,queued_materialization)={let Ok(mailbox)=shared.lock()else{failure(world,"Widget boundary debugger mailbox unavailable".into());return;};(mailbox.enabled,mailbox.active.is_some(),mailbox.error.is_some(),mailbox.queue.iter().any(|queued|matches!(queued.operation,DebugOperation::MaterializeWidget(_))))};
 if failed{return;}
 if active||queued_materialization{if normal&&enabled{super::drain(world);}return;}
 if game.0.widget_materialization_waits_for_saved_operation(){return;}
 if !game.0.has_pending_widget_materialization(){if normal&&enabled{super::drain(world);}return;}
 let operation=match game.0.next_widget_materialization(){Ok(Some(operation))=>operation,Ok(None)=>return,Err(error)=>{failure(world,error.to_string());return;}};
 if enabled{
  let result=(||{let game=world.resource::<VnEngine>();let mut mailbox=shared.lock().map_err(|_|"Widget boundary debugger mailbox unavailable")?;
   if mailbox.active.is_some()||mailbox.queue.iter().any(|queued|matches!(queued.operation,DebugOperation::MaterializeWidget(_))){return Ok(());}
   mailbox.admit(game,DebugOperation::MaterializeWidget(operation),Some(Box::new(|world,outcome|{if !matches!(outcome,DebugOutcome::WidgetMaterialized){return Err("Wrong Widget lifecycle VM receipt".into());}finish_publication(world,outcome)})),0)?;
   let queued=mailbox.queue.pop_back().ok_or("Admitted Widget boundary operation disappeared")?;mailbox.queue.push_front(queued);Ok::<(),String>(())})();
  if let Err(error)=result{failure(world,error);return;}
  if normal{super::drain(world);}
 }else if let Err(error)=world.resource_mut::<VnEngine>().0.execute_widget_materialization(operation){failure(world,error.to_string());}
}
