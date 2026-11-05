//! The existing classic commands adapted to actual standalone Game resources.
use super::*;
use crate::scene_resources::{TargetKind,ResourceTaskKind,EffectChannel};
use crate::scene_tasks::TaskOwner;
impl<R:Renderer> Engine<R>{
 pub(super) fn exec_standalone_sprite(&mut self,command:Statement,native:bool)->Result<(),RuntimeError>{
  self.validate_standalone_timer_target(&self.state)?;
  let mut next=self.state.clone();crate::game_tasks::sync_current(&mut next).map_err(|m|self.scene_resource_error(m))?;
  let id=match &command{Statement::SpriteAnimate{character_id,..}|Statement::SpriteStopAnimation{character_id}|Statement::SpriteEffect{character_id,..}=>character_id,_=>return Err(self.scene_resource_error("Not an actual Game sprite command".into()))};
  if next.sprites.get(id).is_some_and(|sprite|sprite.visible){
   if let Err(message)=next.game_tasks.as_mut().unwrap().resources.ensure_target(TaskOwner::Game,TargetKind::Sprite,id){
    if native{return Err(self.scene_resource_error(message));}
    return self.exec_untracked_historical_sprite(next,command);
   }
  }
  let mut historical=(!native).then(||command.clone());
  match command{
   Statement::SpriteAnimate{character_id,animation,params}=>{
    if next.sprites.get(&character_id).is_some_and(|s|s.visible){
     let target=next.game_tasks.as_ref().unwrap().resources.target(&TaskOwner::Game,TargetKind::Sprite,&character_id).cloned().ok_or_else(||self.scene_resource_error("Game sprite has no actual incarnation".into()))?;
     let effect=crate::sprite_effects::prepare_sprite_animation(&animation,&params).map_err(|m|self.scene_resource_error(m))?;
     crate::scene_resources::supersede_effect_channel(&mut next,&target,EffectChannel::SpriteAnimation).map_err(|m|self.scene_resource_error(m))?;
     let key=crate::scene_resources::resource_key(TaskOwner::Game,ResourceTaskKind::Effect,&format!("sprite/{character_id}/animation"));
     crate::scene_resources::register_effect(&mut next,self.runtime_epoch,key,target,effect).map_err(|m|self.scene_resource_error(m))?;
     if let Some(flags)=next.game_tasks.as_mut().unwrap().legacy_sprite_effects.get_mut(&character_id){flags.animation=false;}
    }
   },
   Statement::SpriteStopAnimation{character_id}=>{
    if let Some(flags)=next.game_tasks.as_mut().unwrap().legacy_sprite_effects.get_mut(&character_id){flags.animation=false;}
    // Resource transitions validate the whole candidate. Retire only the
    // completed animation flag before that gate, retaining other raw effects.
    next.game_tasks.as_mut().unwrap().legacy_sprite_effects.retain(|_,flags|flags.active());
    if let Some(target)=next.game_tasks.as_ref().unwrap().resources.target(&TaskOwner::Game,TargetKind::Sprite,&character_id).cloned(){crate::scene_resources::supersede_effect_channel(&mut next,&target,EffectChannel::SpriteAnimation).map_err(|m|self.scene_resource_error(m))?;}
   },
   Statement::SpriteEffect{character_id,flip_x,flip_y,scale,rotation,tint}=>{
    // Match the narrative adapter: tint is evaluated once on this candidate,
    // even when the authored sprite currently has no visible appearance.
    let tint=tint.map(|text|{
     let parsed=rvn_parser::parse_interpolated_str(&text).map_err(|_|self.scene_resource_error("Invalid sprite color expression".into()))?;
     self.functions.interpolate_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &parsed,&self.vars_for_eval(),&self.lexical_vars_for_eval(),&mut next.random,self.locale.as_ref()).map_err(|e|self.eval_err(e,"Game sprite effect"))
    }).transpose()?;
    if !native{historical=Some(Statement::SpriteEffect{character_id:character_id.clone(),flip_x,flip_y,scale,rotation,tint:tint.clone()});}
    if next.sprites.get(&character_id).is_some_and(|s|s.visible){
     let game=next.game_tasks.as_mut().unwrap();
     let style=game.sprite_styles.entry(character_id.clone()).or_default();
     if let Some(v)=flip_x{style.flip_x=v;}if let Some(v)=flip_y{style.flip_y=v;}
     if let Some(v)=scale{if native||v.is_finite(){style.scale=v;if let Some(flags)=game.legacy_sprite_effects.get_mut(&character_id){flags.scale=false;}}else{game.legacy_sprite_effects.entry(character_id.clone()).or_default().scale=true;}}
     if let Some(v)=rotation{if native||v.is_finite(){style.rotation=v;if let Some(flags)=game.legacy_sprite_effects.get_mut(&character_id){flags.rotation=false;}}else{game.legacy_sprite_effects.entry(character_id.clone()).or_default().rotation=true;}}
     if let Some(v)=tint{
      let mut candidate=style.clone();candidate.tint=Some(v);
      // Bevy's historical callback ignores malformed hexadecimal tint while
      // still applying its other fields. Keep that route, and its exact
      // callback text, without weakening the Native descriptor validator.
      if native||candidate.validate().is_ok(){*style=candidate;}
     }
     style.validate().map_err(|m|self.scene_resource_error(m))?;
    }
   },
   _=>return Err(self.scene_resource_error("Not an actual Game sprite command".into())),
  }
  next.game_tasks.as_mut().unwrap().legacy_sprite_effects.retain(|_,flags|flags.active());
  next.pc+=1;self.commit_audio_state(next)?;
  // Preserve the existing generic Renderer callbacks for raw authored code.
  // They run only after the owned candidate passes the real publication gate.
  if let Some(command)=historical{self.publish_historical_sprite_callback(command);}Ok(())
 }
 /// Preserve the real classic Renderer extension point without representing
 /// unknown payloads as portable tasks. The flags only forbid a lossy save.
 pub(super) fn exec_untracked_historical_sprite(&mut self,mut next:GameState,mut command:Statement)->Result<(),RuntimeError>{
  if let Statement::SpriteEffect{tint:Some(tint),..}=&mut command{
   let parsed=rvn_parser::parse_interpolated_str(tint).map_err(|_|self.scene_resource_error("Invalid sprite color expression".into()))?;
   *tint=self.functions.interpolate_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &parsed,&self.vars_for_eval(),&self.lexical_vars_for_eval(),&mut next.random,self.locale.as_ref()).map_err(|e|self.eval_err(e,"Historical sprite effect"))?;
  }
  let id=match &command{Statement::SpriteAnimate{character_id,..}|Statement::SpriteStopAnimation{character_id}|Statement::SpriteEffect{character_id,..}=>character_id,_=>return Err(self.scene_resource_error("Not a historical sprite callback".into()))};
  if next.sprites.get(id).is_some_and(|sprite|sprite.visible){
   let flags=next.game_tasks.as_mut().unwrap().legacy_sprite_effects.entry(id.clone()).or_default();
   match &command{Statement::SpriteAnimate{..}=>flags.animation=true,Statement::SpriteStopAnimation{..}=>flags.animation=false,Statement::SpriteEffect{..}=>flags.untracked_style=true,_=>unreachable!()}
  }
  next.game_tasks.as_mut().unwrap().legacy_sprite_effects.retain(|_,flags|flags.active());
  next.pc+=1;self.commit_audio_state(next)?;self.publish_historical_sprite_callback(command);Ok(())
 }
 fn publish_historical_sprite_callback(&mut self,command:Statement){match command{
  Statement::SpriteAnimate{character_id,animation,params}=>self.renderer.animate_sprite(&character_id,&animation,&params),
  Statement::SpriteStopAnimation{character_id}=>self.renderer.stop_sprite_animation(&character_id),
  Statement::SpriteEffect{character_id,flip_x,flip_y,scale,rotation,tint}=>self.renderer.set_sprite_effect(&character_id,flip_x,flip_y,scale,rotation,tint.as_deref()),
  _=>unreachable!("Actual historical sprite callback family"),
 }}

}
