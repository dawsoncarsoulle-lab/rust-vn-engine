//! Native classic commands reuse the historical execution bodies below.
//! Only the actual source-bound Game/Scene/Init caller reaches this adapter.
use super::*;
use super::debugger_commands::LegacyIntents;
use std::{rc::Rc,cell::RefCell};
pub(super) fn is_classic(statement:&Statement)->bool {
 matches!(rvn_parser::scheduled_consumer_kind(statement),Some(rvn_parser::ScheduledConsumerKind::Classic(_)))
}
#[derive(Clone,Copy,PartialEq,Eq)]
pub(super) enum ClassicOrigin { Historical, Native }
impl<R:Renderer> Engine<R> {
 pub(super) fn exec_scheduled_classic(&mut self,next:GameState,command:Statement,origin:ClassicOrigin)->Result<(),RuntimeError>{
  // Gather already ran on `next`. Preserve its complete frame and RNG, and
  // share the actual debugger probe rather than inventing another evaluator.
  let source=self.debug_state_source()?;
  let intents=Rc::new(RefCell::new(LegacyIntents::default()));
  let batch=Rc::new(RefCell::new(None));
  let probe=self.functions.debug_probe.clone().unwrap_or_else(crate::eval::debugger_eval::Probe::shared);
  let mut candidate=self.debugger_candidate(probe);
  candidate.functions.debug_probe=self.functions.debug_probe.clone();
  candidate.renderer.1=Some(batch.clone());candidate.renderer.2=Some(intents.clone());
  candidate.state=next;
  candidate.exec_classic_body_origin(command,origin)?;
  intents.borrow().validate().map_err(|e|self.scene_resource_error(e))?;
  if batch.borrow().is_none()&&candidate.narrative.is_some(){candidate.commit_scene_state(candidate.state.clone())?;}
  if batch.borrow().is_none()&&candidate.state.game_tasks.is_some(){candidate.commit_audio_state(candidate.state.clone())?;}
  let mut prepared=batch.borrow_mut().take();
  let proof=if let Some(batch)=&prepared{batch.clone()}else{candidate.build_scene_batch(candidate.state.clone(),candidate.runtime_epoch,candidate.interface_epoch,candidate.video_epoch)?};
  self.renderer.validate_scene_batch(&proof).map_err(|e|self.scene_resource_error(e))?;
  // During Init a real owned batch must carry visual state across the
  // existing Startup preparation boundary; legacy callbacks are not a wire.
  if self.startup.is_some()&&prepared.is_none(){prepared=Some(proof);}

  candidate.consumer_program.validate_state_binding(&candidate.state).map_err(|e|self.eval_err(e,"Classic consumer ownership"))?;
  if !self.accepts_debug_state_source(&source){return Err(self.scene_resource_error("Classic consumer source changed before publication".into()));}
  let state=candidate.state.clone();let timer=candidate.active_timer.clone();
  let epochs=(candidate.runtime_epoch,candidate.interface_epoch,candidate.video_epoch);
  drop(candidate);
  // Every fallible operation is above this boundary. The genuine state and
  // callback sequence are installed only once after complete preflight.
  self.state=state;self.active_timer=timer;
  (self.runtime_epoch,self.interface_epoch,self.video_epoch)=epochs;
  if let Some(batch)=prepared{self.renderer.publish_scene_batch(batch);}
  intents.borrow_mut().publish(&mut self.renderer);
  Ok(())
 }
 pub(super) fn exec_classic_body(&mut self,command:Statement)->Result<(),RuntimeError>{
  self.exec_classic_body_origin(command,ClassicOrigin::Historical)
 }
 fn exec_classic_body_origin(&mut self,command:Statement,origin:ClassicOrigin)->Result<(),RuntimeError>{
  if self.state.game_tasks.is_some(){
   if matches!(command,Statement::Timer{..}|Statement::TimerCancel){return self.exec_standalone_timer(command,origin==ClassicOrigin::Historical);}
   let historical_animation=origin==ClassicOrigin::Historical&&matches!(&command,Statement::SpriteAnimate{animation,params,..} if crate::sprite_effects::prepare_sprite_animation(animation,params).is_err());
   // Historical Renderer callbacks may implement custom animation names. An
   // actual Native source command must have a portable descriptor; that gate
   // cannot be inferred merely from the presence of another Game task.
   if historical_animation{return self.exec_untracked_historical_sprite(self.state.clone(),command);}
   if !historical_animation&&matches!(command,Statement::SpriteAnimate{..}|Statement::SpriteStopAnimation{..}|Statement::SpriteEffect{..}){return self.exec_standalone_sprite(command,origin==ClassicOrigin::Native);}
  }
  match command {
            command @ Statement::MethodCall{..} => return self.exec_native_method_call(command),
            Statement::CharacterCreate { id, display_name } => {
                let mut next = self.state.clone();
                next.layered.declare(id, display_name).map_err(|error| self.scene_resource_error(error))?;
                next.pc += 1;
                if self.narrative.is_some() { return self.commit_scene_state(next); }
                self.state = next;
            }
            Statement::Config { key, value } => {
                if key == "bg" && self.narrative.is_some() {
                    return self.exec_narrative_background(value, Transition::None);
                }
                if key == "bg" {
                    self.state.background_image = value.clone();
                }
                self.state.pc += 1;
            }
            Statement::Scene {
                background,
                transition,
            } => {
                if self.narrative.is_some() {
                    return self.exec_narrative_background(background, transition);
                }
                self.state.motions.tracks.remove("background");
                self.state.background_image = background.clone();
                self.state.last_transition = transition.clone();
                self.renderer.set_background(&background, &transition);
                self.state.pc += 1;
            }
            Statement::CinematicShow { id, transition } => {
                if self.narrative.is_some() { return self.exec_owned_cinematic(Some(id), transition); }
                self.state.cinematic.current = Some(id.clone());
                self.state.cinematic.transition = transition.clone();
                self.renderer.show_cinematic(&id, transition.as_deref());
                self.state.pc += 1;
            }
            Statement::CinematicHide { transition } => {
                if self.narrative.is_some() { return self.exec_owned_cinematic(None, transition); }
                self.state.cinematic.current = None;
                self.state.cinematic.transition = transition.clone();
                self.renderer.hide_cinematic(transition.as_deref());
                self.state.pc += 1;
            }
            Statement::UnlockEnding { id } => {
                self.renderer.unlock_ending(&id);
                self.state.pc += 1;
            }
            Statement::ShowSprite {
                character_id,
                emotion,
                position,
                transition,
            } => {
                if self.narrative.is_some() {
                    return self.exec_show(&character_id, emotion, position, transition);
                }
                self.exec_show(&character_id, emotion, position, transition)?;
                self.state.pc += 1;
            }
            Statement::HideSprite {
                character_id,
                transition,
            } => {
                if self.narrative.is_some() {
                    return self.exec_hide(&character_id, transition);
                }
                self.exec_hide(&character_id, transition)?;
                self.state.pc += 1;
            }
            Statement::MoveSprite {
                character_id,
                position,
                transition,
            } => {
                if self.narrative.is_some() {
                    return self.exec_move(&character_id, position, transition);
                }
                self.exec_move(&character_id, position, transition)?;
                self.state.pc += 1;
            }
            Statement::SpriteAnimate {
                character_id,
                animation,
                params,
            } => {
                if self.narrative.is_some() {
                    let mut next = self.state.clone();
                    if next
                        .sprites
                        .get(&character_id)
                        .is_some_and(|sprite| sprite.visible)
                    {
                        crate::scene_resources::sync_current(&mut next)
                            .map_err(|message| self.scene_resource_error(message))?;
                        let execution = next.scene_execution.as_ref().unwrap();
                        let owner = execution
                            .owner()
                            .map(|activation| crate::scene_tasks::TaskOwner::Scene { activation })
                            .unwrap_or(crate::scene_tasks::TaskOwner::Game);
                        let target = execution
                            .resources
                            .target(
                                &crate::scene_tasks::TaskOwner::Game,
                                crate::scene_resources::TargetKind::Sprite,
                                &character_id,
                            )
                            .cloned()
                            .ok_or_else(|| {
                                self.scene_resource_error(
                                    "Animation target has no incarnation".into(),
                                )
                            })?;
                        let effect =
                            crate::sprite_effects::prepare_sprite_animation(&animation, &params)
                                .map_err(|message| self.scene_resource_error(message))?;
                        crate::scene_resources::supersede_effect_channel(
                            &mut next,
                            &target,
                            crate::scene_resources::EffectChannel::SpriteAnimation,
                        )
                        .map_err(|message| self.scene_resource_error(message))?;
                        let key = crate::scene_resources::resource_key(
                            owner,
                            crate::scene_resources::ResourceTaskKind::Effect,
                            &format!("sprite/{character_id}/animation"),
                        );
                        crate::scene_resources::register_effect(
                            &mut next,
                            self.runtime_epoch,
                            key,
                            target,
                            effect,
                        )
                        .map_err(|message| self.scene_resource_error(message))?;
                    }
                    next.pc += 1;
                    return self.commit_scene_state(next);
                }
                self.renderer
                    .animate_sprite(&character_id, &animation, &params);
                self.state.pc += 1;
            }
            Statement::SpriteStopAnimation { character_id } => {
                if self.narrative.is_some() {
                    let mut next = self.state.clone();
                    if let Some(target) = next
                        .scene_execution
                        .as_ref()
                        .unwrap()
                        .resources
                        .target(
                            &crate::scene_tasks::TaskOwner::Game,
                            crate::scene_resources::TargetKind::Sprite,
                            &character_id,
                        )
                        .cloned()
                    {
                        crate::scene_resources::supersede_effect_channel(
                            &mut next,
                            &target,
                            crate::scene_resources::EffectChannel::SpriteAnimation,
                        )
                        .map_err(|message| self.scene_resource_error(message))?;
                    }
                    next.pc += 1;
                    return self.commit_scene_state(next);
                }
                self.renderer.stop_sprite_animation(&character_id);
                self.state.pc += 1;
            }
            Statement::SpriteEffect {
                character_id,
                flip_x,
                flip_y,
                scale,
                rotation,
                tint,
            } => {
                if self.narrative.is_some() {
                    let mut next = self.state.clone();
                    let widget_objects=self.widget_object_context(&next)?;
                    let tint = tint
                        .map(|text| {
                            let parsed =
                                rvn_parser::parse_interpolated_str(&text).map_err(|_| {
                                    self.scene_resource_error(
                                        "Invalid sprite color expression".into(),
                                    )
                                })?;
                            self.functions
                                .interpolate_scoped_with_locale_with_widget_objects(widget_objects.as_ref(), 
                                    &parsed,
                                    &self.vars_for_eval(),
                                    &self.lexical_vars_for_eval(),
                                    &mut next.random, self.locale.as_ref(),
                                )
                                .map_err(|error| self.eval_err(error, "Sprite effect"))
                        })
                        .transpose()?;
                    if next
                        .sprites
                        .get(&character_id)
                        .is_some_and(|sprite| sprite.visible)
                    {
                        let style = next
                            .scene_execution
                            .as_mut()
                            .unwrap()
                            .sprite_styles
                            .entry(character_id)
                            .or_default();
                        if let Some(value) = flip_x {
                            style.flip_x = value;
                        }
                        if let Some(value) = flip_y {
                            style.flip_y = value;
                        }
                        if let Some(value) = scale {
                            style.scale = value;
                        }
                        if let Some(value) = rotation {
                            style.rotation = value;
                        }
                        if let Some(value) = tint {
                            style.tint = Some(value);
                        }
                        style
                            .validate()
                            .map_err(|message| self.scene_resource_error(message))?;
                    }
                    next.pc += 1;
                    return self.commit_scene_state(next);
                }
                let widget_objects=self.widget_object_context(&self.state)?;
                let tint = tint
                    .map(|text| {
                        let parsed = rvn_parser::parse_interpolated_str(&text).map_err(|_| {
                            self.eval_err(
                                EvalError::TypeMismatch {
                                    op: "tint".into(),
                                    left: "color expression".into(),
                                    right: text.clone(),
                                },
                                "SpriteEffect",
                            )
                        })?;
                        self.functions
                            .interpolate_scoped_with_locale_with_widget_objects(widget_objects.as_ref(), 
                                &parsed,
                                &self.vars_for_eval(),
                                &self.lexical_vars_for_eval(),
                                &mut self.state.random, self.locale.as_ref(),
                            )
                            .map_err(|e| self.eval_err(e, "SpriteEffect"))
                    })
                    .transpose()?;
                self.renderer.set_sprite_effect(
                    &character_id,
                    flip_x,
                    flip_y,
                    scale,
                    rotation,
                    tint.as_deref(),
                );
                self.state.pc += 1;
            }
            Statement::Timer {
                duration_secs,
                action,
            } => {
                if self.narrative.is_some() {
                    let parts: Vec<_> = action.split_whitespace().collect();
                    if let ["jump" | "call", target] = parts.as_slice() {
                        self.resolve(target)?;
                    }
                    let mut next = self.state.clone();
                    let execution = next.scene_execution.as_mut().unwrap();
                    let tasks = match &mut execution.active {
                        Some(active) => &mut active.tasks,
                        None => &mut execution.game_tasks,
                    };
                    tasks
                        .set_timer(f64::from(duration_secs), action)
                        .map_err(|message| {
                            self.eval_err(EvalError::InvalidFunction(message), "Scene timer")
                        })?;
                    next.pc += 1;
                    return self.commit_scene_state(next);
                }
                self.active_timer = Some((duration_secs, action.clone(), 0.0));
                self.state.pc += 1;
            }
            Statement::TimerCancel => {
                if self.narrative.is_some() {
                    let mut next = self.state.clone();
                    let execution = next.scene_execution.as_mut().unwrap();
                    let tasks = match &mut execution.active {
                        Some(active) => &mut active.tasks,
                        None => &mut execution.game_tasks,
                    };
                    tasks.timer = None;
                    next.pc += 1;
                    return self.commit_scene_state(next);
                }
                self.active_timer = None;
                self.state.pc += 1;
            }
            Statement::TypewriterSet { enabled } => {
                return self.commit_typewriter_setting(self.state.clone(),enabled);
            }
            Statement::TypewriterSpeed { chars_per_sec } => {
                return self.commit_typewriter_speed(self.state.clone(),chars_per_sec);
            }
   _=>return Err(self.scene_resource_error("No executable classic command adapter".into())),
  }
  Ok(())
 }
}
