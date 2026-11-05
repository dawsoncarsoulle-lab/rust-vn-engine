//! Standalone Game adapter: one saved timer in the existing frame transaction.
use super::*;
impl<R:Renderer> Engine<R>{
 pub(super) fn validate_standalone_timer_target(&self,state:&GameState)->Result<(),RuntimeError>{
  self.validate_standalone_timer_target_with_context(state,self.active_timer.as_ref())
 }
 pub(super) fn validate_standalone_timer_target_with_context(&self,state:&GameState,timer_context:Option<&(f32,String,f32)>)->Result<(),RuntimeError>{
  crate::game_tasks::validate(state).map_err(|m|self.scene_resource_error(m))?;
  if let (Some(game),Some((duration,action,_)))=(&state.game_tasks,timer_context){
   let mut portable=crate::scene_tasks::SceneTasks::default();
   if game.tasks.timer.is_some()||!self.consumer_program.admits_historical_timer(*duration,action)||portable.set_timer(f64::from(*duration),action.clone()).is_ok(){return Err(self.scene_resource_error("Game timer has conflicting or unauthored historical authority".into()));}
  }
  if state.game_tasks.is_some(){self.consumer_program.validate_emitted(&self.script).and_then(|_|self.consumer_program.validate_state_binding(state)).map_err(|e|self.eval_err(e,"Owned Game task source"))?;}
  if let Some(timer)=state.game_tasks.as_ref().filter(|g|!g.historical_timer).and_then(|g|g.tasks.timer.as_ref()){
   let parts:Vec<_>=timer.action.split_whitespace().collect();
   if let ["jump"|"call",target]=parts.as_slice(){self.resolve(target)?;}else{return Err(self.scene_resource_error("Invalid owned Game timer action".into()));}
  }Ok(())
 }
 pub(super) fn exec_standalone_timer(&mut self,command:Statement,historical:bool)->Result<(),RuntimeError>{
  self.validate_standalone_timer_target(&self.state)?;
  let mut next=self.state.clone();let mut legacy=None;
  match command{
   Statement::Timer{duration_secs,action}=>{
    let parts:Vec<_>=action.split_whitespace().collect();
    if let ["jump"|"call",target]=parts.as_slice(){if !historical{self.resolve(target)?;}}else{return Err(self.scene_resource_error("Invalid owned Game timer action".into()));}
    let tasks=&mut next.game_tasks.as_mut().ok_or_else(||self.scene_resource_error("Missing actual Game task owner".into()))?.tasks;
    if historical&&(!duration_secs.is_finite()||action.len()>4096||!crate::scene_tasks::valid_timer_action(&action)){
     tasks.timer=None;next.game_tasks.as_mut().unwrap().historical_timer=false;legacy=Some((duration_secs,action,0.0));
    }else{
     tasks.set_timer(f64::from(duration_secs),action).map_err(|message|self.scene_resource_error(message))?;
     next.game_tasks.as_mut().unwrap().historical_timer=historical;
    }
   },
   Statement::TimerCancel=>{next.game_tasks.as_mut().ok_or_else(||self.scene_resource_error("Missing actual Game task owner".into()))?.tasks.timer=None;next.game_tasks.as_mut().unwrap().historical_timer=false;},
   _=>return Err(self.scene_resource_error("Not an owned Game timer command".into())),
  }
  next.pc+=1;self.commit_audio_state_with_timer(next,legacy)
 }
 pub(super) fn plan_standalone_timer_action(&self,mut next:GameState,action:&str,historical:bool)->Result<GameState,RuntimeError>{
  self.validate_standalone_timer_target(&next)?;
  let parts:Vec<_>=action.split_whitespace().collect();
  let [operation,target]=parts.as_slice()else{return Err(self.scene_resource_error("Invalid owned Game timer action".into()));};
  let destination=self.resolve(target)?;
  match *operation{
   "jump"=>{},
   "call"=>{
    if next.call_stack.len()>=128{return Err(self.eval_err(EvalError::ExecutionLimit{limit:"128 nested narrative calls"},"Game timer Call"));}
    // Match the existing standalone timer/Call continuation rule. A displayed
    // interaction is skipped on return; an unexecuted instruction is retained.
    let displayed=matches!(Self::consumer_leaf(self.script.get(next.pc)),Some(Statement::Dialogue{..}|Statement::Choice{..}|Statement::Imagemap{..}));
    next.call_stack.push(if historical||displayed{next.pc+1}else{next.pc});
   },
   _=>return Err(self.scene_resource_error("Invalid owned Game timer operation".into())),
  }
  next.pc=destination;next.display_random_pc=None;
  // Retained display records remain historical; live choice/map input leases
  // cannot be submitted after a timer transferred away from their current PC.
  next.consumer_choice=None;next.consumer_imagemap=None;
  Ok(next)
 }
 pub(super) fn fire_standalone_timer(&mut self,token:&crate::scene_tasks::TaskToken)->Result<bool,RuntimeError>{
  if self.startup.is_some()||token.runtime_epoch!=self.runtime_epoch||token.key.owner!=crate::scene_tasks::TaskOwner::Game||token.key.local_id!="timer"{return Ok(false);}
  let Some(task)=self.state.game_tasks.as_ref().and_then(|g|g.tasks.timer.as_ref()).filter(|t|t.revision==token.revision&&t.elapsed>=t.duration)else{return Ok(false);};
  let action=task.action.clone();let historical=self.state.game_tasks.as_ref().unwrap().historical_timer;let mut next=self.state.clone();next.game_tasks.as_mut().unwrap().tasks.timer=None;next.game_tasks.as_mut().unwrap().historical_timer=false;
  let next=self.plan_standalone_timer_action(next,&action,historical)?;self.commit_audio_state(next)?;Ok(true)
 }
}
