//! The real standalone Game lifetime. No scene, audio namespace or scheduler.
use crate::{GameState,scene_tasks::{SceneTasks,TaskOwner},scene_resources::{ResourceRegistry,TargetKind,ResourceTaskKind,EffectParameters,CancellationReason}};
use serde::{Serialize,Deserialize};
#[derive(Clone,Debug,Default,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameTasksState {
 pub tasks:SceneTasks,
 // Authored raw timers keep delayed label resolution on this same saved slot.
 #[serde(default,skip_serializing_if="is_false")]
 pub historical_timer:bool,
 // These flags record actual historical callbacks without inventing a saved
 // animation backend. Runtime keeps accepting them; portable capture refuses.
 #[serde(default,skip_serializing_if="std::collections::HashMap::is_empty")]
 pub legacy_sprite_effects:std::collections::HashMap<String,LegacySpriteEffects>,
 pub resources:ResourceRegistry,
 pub sprite_styles:std::collections::HashMap<String,crate::sprite_effects::SpriteStyle>,
}
#[derive(Clone,Debug,Default,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacySpriteEffects {pub animation:bool,pub scale:bool,pub rotation:bool,pub untracked_style:bool}
impl LegacySpriteEffects {pub fn active(&self)->bool{self.animation||self.scale||self.rotation||self.untracked_style}}
fn is_false(value:&bool)->bool{!*value}
impl GameTasksState {
 pub fn validate(&self)->Result<(),String>{
  self.tasks.validate()?;
  if self.historical_timer&&self.tasks.timer.is_none(){return Err("Historical timer provenance has no actual saved timer".into());}
  if self.tasks.resources_parked||self.tasks.suspended_motions!=Default::default()||self.tasks.suspended_videos!=Default::default()||self.tasks.suspended_ui!=Default::default(){return Err("Standalone Game tasks cannot contain parked Scene/Host snapshots".into());}
  self.resources.validate_game_resources()?;
  if !self.resources.waits.is_empty()||!self.resources.cancelled_waits.is_empty()||!self.resources.music_checkpoints.is_empty(){return Err("Game sprite authority cannot carry waits or audio checkpoints".into());}
  if self.resources.targets.iter().any(|r|r.owner!=TaskOwner::Game||r.identity.kind!=TargetKind::Sprite){return Err("Game sprite registry has a foreign owner or target kind".into());}
  for task in &self.resources.tasks{
   if task.key!=crate::scene_resources::resource_key(TaskOwner::Game,ResourceTaskKind::Effect,&format!("sprite/{}/animation",task.target.alias))||task.key.owner!=TaskOwner::Game||task.kind!=ResourceTaskKind::Effect||task.target.kind!=TargetKind::Sprite||task.parked||!task.dependencies.is_empty()||task.wait.is_some()||task.audio.is_some()||!task.effect.as_ref().is_some_and(|e|e.presentation.is_none()&&matches!(e.parameters,EffectParameters::SpriteAnimation{..})){return Err("Game sprite operation has no actual portable animation descriptor".into());}
  }
  if self.sprite_styles.len()>crate::scene_resources::MAX_RESOURCES{return Err("Too many Game sprite styles".into());}
  for(id,style)in &self.sprite_styles{if !self.resources.targets.iter().any(|r|r.identity.alias==*id){return Err("Game sprite style has no live target".into());}style.validate()?;}
  Ok(())
 }
}
pub(crate) fn validate(state:&GameState)->Result<(),String>{
 if let Some(game)=&state.game_tasks{
  if state.scene_execution.is_some(){return Err("Scene and standalone Game task authorities cannot coexist".into());}
  game.validate()?;
  let live=state.sprites.iter().filter(|(_,s)|s.visible).map(|(id,_)|id.as_str()).collect::<std::collections::BTreeSet<_>>();
  if game.resources.targets.iter().any(|r|!live.contains(r.identity.alias.as_str())){return Err("Game sprite incarnations differ from actual visible sprites".into());}
  if game.legacy_sprite_effects.iter().any(|(id,flags)|!flags.active()||!live.contains(id.as_str())){return Err("Historical sprite callback differs from actual visible sprite".into());}
 }
 Ok(())
}
/// Normalize only the real standalone Game sprite model, transaction-local.
pub(crate) fn sync_current(state:&mut GameState)->Result<(),String>{
 let Some(game)=&mut state.game_tasks else{return Ok(());};
 let live=state.sprites.iter().filter(|(_,s)|s.visible).map(|(id,_)|id.clone()).collect::<std::collections::BTreeSet<_>>();
 let removed=game.resources.targets.iter().filter(|r|!live.contains(&r.identity.alias)).map(|r|r.identity.alias.clone()).collect::<Vec<_>>();
 for id in removed{game.resources.remove_target(&TaskOwner::Game,TargetKind::Sprite,&id,CancellationReason::TargetRemoved)?;game.sprite_styles.remove(&id);game.legacy_sprite_effects.remove(&id);}
 // Showing an unrelated raw sprite does not opt it into a bounded Native
 // identity. Animate/Stop/Effect admit their actual target when they need it.
 game.legacy_sprite_effects.retain(|id,_|live.contains(id));
 Ok(())
}
/// Source-derived only; pure Legacy scripts do not gain a new state field.
pub(crate) fn has_native_tasks(script:&[rvn_parser::Statement])->bool{
 let mut blocks=vec![script];while let Some(block)=blocks.pop(){for s in block{
  if matches!(s,rvn_parser::Statement::ScheduledConsumer{version:2,body,..} if body.first().is_some_and(is_task_command)){return true;}
  blocks.extend(crate::consumer_schedule::statement_children(s));
 }}false
}
pub(crate) fn is_task_command(s:&rvn_parser::Statement)->bool{matches!(s,rvn_parser::Statement::Timer{..}|rvn_parser::Statement::TimerCancel|rvn_parser::Statement::SpriteAnimate{..}|rvn_parser::Statement::SpriteStopAnimation{..}|rvn_parser::Statement::SpriteEffect{..})}
impl GameState{
 /// Choose by actual target family. Never fall back from sprites to audio.
 pub fn effect_resource_registry(&self,kind:TargetKind)->Option<&ResourceRegistry>{
  if let Some(scene)=&self.scene_execution{return Some(&scene.resources);}
  match kind{TargetKind::Sprite=>self.game_tasks.as_ref().map(|g|&g.resources),TargetKind::Music|TargetKind::SfxPlayback|TargetKind::VoicePlayback|TargetKind::PresentationRetired=>self.game_audio.as_ref(),_=>None}
 }
 pub(crate) fn effect_resource_registry_mut(&mut self,kind:TargetKind)->Option<&mut ResourceRegistry>{
  if let Some(scene)=&mut self.scene_execution{return Some(&mut scene.resources);}
  match kind{TargetKind::Sprite=>self.game_tasks.as_mut().map(|g|&mut g.resources),TargetKind::Music|TargetKind::SfxPlayback|TargetKind::VoicePlayback|TargetKind::PresentationRetired=>self.game_audio.as_mut(),_=>None}
 }
 pub fn sprite_styles(&self)->Option<&std::collections::HashMap<String,crate::sprite_effects::SpriteStyle>>{self.scene_execution.as_ref().map(|s|&s.sprite_styles).or_else(||self.game_tasks.as_ref().map(|g|&g.sprite_styles))}
}

/// Exact authored raw timer declarations; Native wrapper bodies are excluded.
/// This is source provenance for the one timer slot, not an execution allowlist.
pub(crate) fn historical_timer_declarations(script:&[rvn_parser::Statement])->std::collections::BTreeSet<(u64,String)>{
 let mut found=std::collections::BTreeSet::new();let mut blocks=vec![script];
 while let Some(block)=blocks.pop(){for statement in block{
  if matches!(statement,rvn_parser::Statement::ScheduledConsumer{version:2,..}){continue;}
  if let rvn_parser::Statement::Timer{duration_secs,action}=statement{found.insert((f64::from(*duration_secs).to_bits(),action.clone()));}
  blocks.extend(crate::consumer_schedule::statement_children(statement));
 }}found
}

/// Runtime compatibility is not permission to silently omit renderer state.
pub(crate) fn validate_portable(state:&GameState)->Result<(),String>{
 if state.game_tasks.as_ref().is_some_and(|game|!game.legacy_sprite_effects.is_empty()){return Err("Historical sprite callback has no portable Game effect descriptor; stop or replace it before Save15".into());}Ok(())
}
