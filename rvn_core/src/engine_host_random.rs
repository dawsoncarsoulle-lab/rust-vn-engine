//! Runtime-only Host randomness. Planning never advances the saved Story RNG.
//! This private stage is a child of engine.rs; it is not installed.
use super::*;
use crate::random::RandomState;
use crate::scene_resources::{TargetIdentity, TargetKind};
use crate::scene_tasks::TaskOwner;
use crate::ui_residency::ProjectionResidence;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HostRandomState {
    epoch: Option<u64>,
    seed: RandomState,
    streams: BTreeMap<String, RandomState>,
}

impl Default for HostRandomState {
    fn default() -> Self {
        Self {
            epoch: None,
            seed: RandomState::fresh(),
            streams: BTreeMap::new(),
        }
    }
}

impl HostRandomState {
    fn stream(&mut self, epoch: u64, role: &str) -> Result<RandomState, String> {
        if rvn_ui::PageRole::from_id(role).is_none() {
            return Err("Invalid autonomous Host random owner".into());
        }
        if self.epoch != Some(epoch) {
            self.epoch = Some(epoch);
            self.streams.clear();
        }
        // One persistent stream per real Host role in this Engine instance.
        // The seed is independent of Story randomness and is not save data.
        Ok(*self.streams.entry(role.into()).or_insert_with(|| {
            let seed = self.seed.next_u64();
            RandomState::seeded(role.bytes().fold(seed, |seed, byte| {
                seed.wrapping_mul(1099511628211) ^ u64::from(byte)
            }))
        }))
    }
}

#[derive(Default)]
pub(super) struct HostRandomCandidate {
    active: bool,
    value: HostRandomState,
    pub(super) historical_audio: Option<Result<crate::audio_checkpoint::HistoricalGameSnapshot,String>>,
    retired_widget_objects:Vec<crate::widget_backend_0956::WidgetBackendObjectIdentity>,
}

pub(super) type HostRandomPlanning = Arc<Mutex<HostRandomCandidate>>;

struct PlanningGuard(HostRandomPlanning);
impl Drop for PlanningGuard {
    fn drop(&mut self) {
        // The planning value is disposable; a panic never installs it.
        let mut planning=self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        planning.active=false;
        planning.historical_audio=None;
        planning.retired_widget_objects.clear();
    }
}

#[derive(Clone)]
pub(super) struct UiRandomAuthority {
    identity: TargetIdentity,
    owner: TaskOwner,
    order: u64,
    role: Option<rvn_ui::PageRole>,
    residence: ProjectionResidence,
    random: RandomState,
}

enum RandomDestination {
    Story,
    Ui(SharedUiRandom),
    AutonomousHost(String),
}

// This context lives only in the callback's candidate command queue. A clone
// shares its cursor; closing/replacing its source cannot reset the cursor or
// transfer it to an unrelated instance that happens to have the same name.
pub(super) type SharedUiRandom = Rc<RefCell<UiRandomAuthority>>;
pub(super) struct CallbackCommands {
    pub(super) commands: Vec<crate::eval::UiCommand>,
    pub(super) owner: TaskOwner,
    pub(super) random: Option<SharedUiRandom>,
}

impl<R: Renderer> Engine<R> {
 pub(super) fn widget_retirement_already_planned(&self,object:&crate::widget_backend_0956::WidgetBackendObjectIdentity)->Result<bool,RuntimeError>{let planning=self.host_random_planning.lock().map_err(|_|self.scene_resource_error("Widget lifecycle planning unavailable".into()))?;Ok(planning.retired_widget_objects.contains(object))}
 pub(super) fn record_widget_retirement(&self,object:&crate::widget_backend_0956::WidgetBackendObjectIdentity)->Result<bool,RuntimeError>{
  let mut planning=self.host_random_planning.lock().map_err(|_|self.scene_resource_error("Widget lifecycle planning unavailable".into()))?;
  if !planning.active{return Err(self.scene_resource_error("Widget lifecycle requires its actual runtime transaction".into()));}
  if planning.retired_widget_objects.contains(object){return Ok(false);}
  if object.generation==0||object.owner.lineage()!=self.instance_id()||object.owner.epoch()!=self.runtime_epoch()||planning.retired_widget_objects.len()>=32{return Err(self.scene_resource_error("Widget lifecycle object is outside its original bounded runtime transaction".into()));}
  planning.retired_widget_objects.push(object.clone());Ok(true)
 }
    /// A small runtime transaction shared by the actual UI/media planning paths.
    /// Helpers remain immutable Engine readers and mutate only their GameState
    /// candidate plus this cloned runtime-only value. Only the outer accepted
    /// entry installs the Host value; legacy and nested wrappers keep their API.
    pub(super) fn with_host_random_transaction<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let nested = self
            .host_random_planning
            .lock()
            .map_err(|_| self.host_random_error("Host random planning lock is unavailable"))?
            .active;
        if nested {
            let before = {let planning=self.host_random_planning.lock().map_err(|_|self.host_random_error("Host random planning lock is unavailable"))?;(planning.value.clone(),planning.retired_widget_objects.clone())};
            let result = operation(self);
            if result.is_err() {
                // A nested refusal cannot leak draws if its caller handles it.
                let mut planning=self.host_random_planning.lock().unwrap_or_else(|poisoned|poisoned.into_inner());planning.value=before.0;planning.retired_widget_objects=before.1;
            }
            return result;
        }
        let historical_audio=if self.narrative.is_some()||self.state.game_audio.is_some(){Some(self.capture_historical_audio())}else{None};
        {
            let mut planning = self
                .host_random_planning
                .lock()
                .map_err(|_| self.host_random_error("Host random planning lock is unavailable"))?;
            planning.value = self.host_random.clone();
            planning.historical_audio=historical_audio;
            planning.retired_widget_objects.clear();
            planning.active = true;
        }
        let guard = PlanningGuard(Arc::clone(&self.host_random_planning));
        let result = operation(self);
        if result.is_ok() {
            // No fallible operation follows the accepted Core publication.
            // The guard owns the context; helpers cannot remove its value.
            self.host_random = guard
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .value
                .clone();
        }
        drop(guard);
        result
    }

    pub(super) fn reset_host_random_instance(&mut self) {
        self.host_random = Default::default();
        let mut planning = self
            .host_random_planning
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if planning.active {
            planning.value = self.host_random.clone();
        }
    }

    fn host_random_error(&self, message: &str) -> RuntimeError {
        self.eval_err(
            EvalError::InvalidFunction(message.into()),
            "Host random authority",
        )
    }

    pub(super) fn capture_ui_random_authority(
        &self,
        next: &GameState,
        identity: &TargetIdentity,
    ) -> Result<UiRandomAuthority, RuntimeError> {
        let resident = crate::ui_residency::resident_interface(next, identity)
            .map_err(|message| self.host_random_error(&message))?;
        Ok(UiRandomAuthority {
            identity: resident.identity,
            owner: resident.owner,
            order: resident.instance.order,
            role: resident.instance.host_role,
            residence: resident.residence,
            random: resident.instance.random,
        })
    }

    pub(super) fn capture_visible_ui_random(
        &self,
        next: &GameState,
        screen: &str,
    ) -> Result<UiRandomAuthority, RuntimeError> {
        let identity = crate::ui_residency::visible_interface_target(next, screen)
            .map_err(|message| self.host_random_error(&message))?;
        self.capture_ui_random_authority(next, &identity)
    }

    fn store_ui_random(
        &self,
        next: &mut GameState,
        authority: &UiRandomAuthority,
        random: RandomState,
    ) -> Result<(), RuntimeError> {
        // Revalidate identity first; order/role are not substitutes for a real
        // incarnation. The readonly borrow ends before the scoped mutable one.
        let current = self.capture_ui_random_authority(next, &authority.identity)?;
        if current.owner != authority.owner
            || current.order != authority.order
            || current.role != authority.role
            || current.residence != authority.residence
        {
            return Err(self.host_random_error("Host handler's exact UI instance changed"));
        }
        let source = match authority.residence {
            ProjectionResidence::Visible => &mut next.ui,
            ProjectionResidence::Parked => {
                let TaskOwner::Scene { activation } = &authority.owner else {
                    return Err(self.host_random_error("A non-Scene UI cannot be parked"));
                };
                let execution = next.scene_execution.as_mut().ok_or_else(|| {
                    self.host_random_error("Host UI random snapshot has no scene execution")
                })?;
                let active_matches = execution
                    .active
                    .as_ref()
                    .is_some_and(|scene| scene.id == *activation && scene.tasks.resources_parked);
                let parent = if active_matches {
                    execution.active.as_mut().unwrap()
                } else {
                    execution
                        .frames
                        .iter_mut()
                        .filter_map(|frame| frame.parked.as_mut())
                        .find(|scene| scene.id == *activation && scene.tasks.resources_parked)
                        .ok_or_else(|| {
                            self.host_random_error("Exact parked UI random snapshot is absent")
                        })?
                };
                &mut parent.tasks.suspended_ui
            }
        };
        let instance = source
            .screens
            .iter_mut()
            .find(|screen| {
                screen.name == authority.identity.alias
                    && screen.order == authority.order
                    && screen.host_role == authority.role
            })
            .ok_or_else(|| self.host_random_error("Exact Host UI random instance disappeared"))?;
        instance.random = random;
        Ok(())
    }

    fn evaluate_owned_random<T>(
        &self,
        next: &mut GameState,
        owner: &TaskOwner,
        ui: Option<&SharedUiRandom>,
        allow_removed_source: bool,
        operation: impl FnOnce(&mut GameState, &mut RandomState) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let (destination, mut random) =
            if (next.scene_execution.is_some()||next.ui.instance_registry.is_some()) && matches!(owner, TaskOwner::Host { .. }) {
                if let Some(source) = ui {
                    let random = source.borrow().random;
                    (RandomDestination::Ui(Rc::clone(source)), random)
                } else {
                    let TaskOwner::Host { role } = owner else {
                        unreachable!()
                    };
                    let mut planning = self.host_random_planning.lock().map_err(|_| {
                        self.host_random_error("Host random planning lock is unavailable")
                    })?;
                    if !planning.active {
                        return Err(self.host_random_error(
                            "Autonomous Host handler requires a runtime random transaction",
                        ));
                    }
                    let random = planning
                        .value
                        .stream(self.runtime_epoch, role)
                        .map_err(|message| self.host_random_error(&message))?;
                    (RandomDestination::AutonomousHost(role.clone()), random)
                }
            } else {
                (RandomDestination::Story, next.random)
            };
        let result = operation(next, &mut random)?;
        match destination {
            RandomDestination::Story => next.random = random,
            RandomDestination::Ui(source) => {
                let authority = source.borrow().clone();
                let still_resident = next.ui.instance_registry.as_ref().is_some_and(|registry|registry.targets.iter().any(|record|record.identity==authority.identity))||next.scene_execution.as_ref().is_some_and(|execution| {
                    execution
                        .resources
                        .targets
                        .iter()
                        .any(|record| record.identity == authority.identity)
                });
                let exact_screen = |screen: &crate::ui::ScreenInstance| {
                    screen.name == authority.identity.alias
                        && screen.order == authority.order
                        && screen.host_role == authority.role
                };
                let physically_present =
                    next.ui.screens.iter().any(exact_screen)
                        || next.scene_execution.as_ref().is_some_and(|execution| {
                            execution.active.iter().any(|scene| {
                                scene.tasks.suspended_ui.screens.iter().any(exact_screen)
                            }) || execution
                                .frames
                                .iter()
                                .filter_map(|frame| frame.parked.as_ref())
                                .any(|scene| {
                                    scene.tasks.suspended_ui.screens.iter().any(exact_screen)
                                })
                        });
                if still_resident && physically_present {
                    self.store_ui_random(next, &authority, random)?;
                } else if !allow_removed_source {
                    return Err(self.host_random_error(
                        "Callback's exact UI random source disappeared during evaluation",
                    ));
                }
                // Closed source continues only in this disposable command list.
                // It is never saved into a replacement interface or Story RNG.
                source.borrow_mut().random = random;
            }
            RandomDestination::AutonomousHost(role) => {
                let mut planning = self.host_random_planning.lock().map_err(|_| {
                    self.host_random_error("Host random planning lock is unavailable")
                })?;
                if !planning.active {
                    return Err(
                        self.host_random_error("Autonomous Host random candidate disappeared")
                    );
                }
                planning.value.streams.insert(role, random);
            }
        }
        Ok(result)
    }

    pub(super) fn ui_event_owned_budgeted(
        &self,
        next: &mut GameState,
        event: &crate::ui::UiInput,
        component: &rvn_ui::programmable::Component,
        owner: &TaskOwner,
        closed: Option<UiRandomAuthority>,
        closed_instance: Option<super::host_consumer::ClosedInterface>,
        budget: &mut crate::ui::CanvasBudget,
    ) -> Result<CallbackCommands, RuntimeError> {
        let source_already_closed = closed.is_some();
        let mut native_frame=self.native_event_frame(next,event,component,owner,closed_instance.as_ref())?;
        let mut macro_frame=match self.ui_library.event_handler(event,component){
            Some(handler)=>self.widget_macro_handler_frame(next,&event.screen,handler,owner,closed_instance.as_ref())?,None=>None,
        };
        let authority = if (next.scene_execution.is_some()||next.ui.instance_registry.is_some()) && matches!(owner, TaskOwner::Host { .. })
        {
            Some(match closed {
                Some(source) => source,
                None => self.capture_visible_ui_random(next, &event.screen)?,
            })
        } else {
            None
        };
        let random_context = authority.map(|authority| Rc::new(RefCell::new(authority)));
        let commands = self.evaluate_owned_random(
            next,
            owner,
            random_context.as_ref(),
            source_already_closed,
            |next, random| {
                let mut globals = self.global_vars_for_state(next);
                let commands = self.ui_library.with_locale(self.locale.as_ref())
                    .event_budgeted_frame(
                        &mut next.ui,
                        &mut globals,
                        event,
                        component,
                        &self.functions,
                        random,
                        budget,
                        native_frame.as_mut().map(|frame|&mut frame.frame),
                        macro_frame.as_mut(),
                    )
                    .map_err(|error| self.eval_err(error, "Owned interface event"))?;
                next.vars = Self::without_persistent_globals(globals);
                if let Some(frame)=native_frame.take(){self.store_native_event_frame(next,frame,closed_instance.is_some())?;}
                if let Some(frame)=macro_frame.take(){self.store_widget_macro_frame(next,frame,closed_instance.is_some())?;}
                Ok(commands)
            },
        )?;
        Ok(CallbackCommands {
            commands,
            owner: owner.clone(),
            random: random_context,
        })
    }

    /// Internal owner VM dispatch. A renderer/native event must first pass its
    /// exact published owner/source/geometry admission port; this method does
    /// not itself mint any external focus or input capability.
    pub(super) fn owner_handler_owned_budgeted(
        &self,next:&mut GameState,screen:&str,kind:rvn_ui::programmable::HostEventKind,
        value:Value,owner:&TaskOwner,closed:Option<UiRandomAuthority>,
        closed_instance:Option<super::host_consumer::ClosedInterface>,budget:&mut crate::ui::CanvasBudget,
    )->Result<CallbackCommands,RuntimeError>{self.owner_handler_result_owned_budgeted(next,screen,kind,value,owner,closed,closed_instance,budget).map(|(commands,_)|commands)}
    /// Preserve event reply data from the same actual Native/Legacy handler VM.
    /// No reply grants a backend capability until its real path is admitted.
    pub(super) fn owner_handler_result_owned_budgeted(
        &self,next:&mut GameState,screen:&str,kind:rvn_ui::programmable::HostEventKind,
        value:Value,owner:&TaskOwner,closed:Option<UiRandomAuthority>,
        closed_instance:Option<super::host_consumer::ClosedInterface>,budget:&mut crate::ui::CanvasBudget,
    )->Result<(CallbackCommands,Option<Value>),RuntimeError>{
        let instance=if let Some(closed)=&closed_instance{
            closed.owner_instance(screen,owner).map_err(|s|self.host_random_error(s))?
        }else{
            let instance=next.ui.screens.iter().find(|instance|instance.name==screen).ok_or_else(||self.host_random_error("Owner callback has no resident instance"))?;
            if next.scene_execution.is_some()||next.ui.instance_registry.is_some(){
                let target=crate::ui_residency::visible_interface_target(next,screen).map_err(|s|self.host_random_error(&s))?;
                let resident=crate::ui_residency::resident_interface(next,&target).map_err(|s|self.host_random_error(&s))?;
                if resident.owner!=*owner||resident.residence!=ProjectionResidence::Visible{return Err(self.host_random_error("Owner callback differs from actual visible lifecycle owner"));}
            }else{
                let expected=instance.host_role.map(|role|TaskOwner::Host{role:role.id().into()}).unwrap_or_else(||Self::command_owner(next));
                if expected!=*owner{return Err(self.host_random_error("Owner callback differs from legacy lifecycle owner"));}
            }
            instance
        };
        instance.owner_properties.validate().map_err(|s|self.host_random_error(&s))?;
        let role=instance.host_role;
        let Some(handler)=instance.owner_properties.events.get(&kind).cloned()else{return Ok((CallbackCommands{commands:vec![],owner:owner.clone(),random:None},None));};
        self.widget_classes.validate_owner_handler(instance.widget_class.as_ref(),&handler).map_err(|message|self.host_random_error(&message))?;
        let mut frame=self.native_named_instance_handler_frame(next,screen,&handler,owner,closed_instance.as_ref())?;
        let mut macro_frame=self.widget_macro_handler_frame(next,screen,&handler,owner,closed_instance.as_ref())?;
        let already_closed=closed.is_some();
        let authority=if(next.scene_execution.is_some()||next.ui.instance_registry.is_some())&&matches!(owner,TaskOwner::Host{..}){
            Some(match closed{Some(authority)=>authority,None=>self.capture_visible_ui_random(next,screen)?})
        }else{None};
        let random_context=authority.map(|source|Rc::new(RefCell::new(source)));
        let(mut commands,reply)=self.evaluate_owned_random(next,owner,random_context.as_ref(),already_closed,|next,random|{
            let(variables,commands,reply)=self.ui_library.with_locale(self.locale.as_ref()).owner_event_result_budgeted_instance_frames(
                &handler,value,&self.functions,&self.global_vars_for_state(next),random,budget,frame.as_mut().map(|frame|&mut frame.frame),macro_frame.as_mut(),
            ).map_err(|e|self.eval_err(e,"UserWidget owner handler"))?;
            next.vars=Self::without_persistent_globals(variables);
            if let Some(frame)=frame.take(){self.store_native_event_frame(next,frame,closed_instance.is_some())?;}
            if let Some(frame)=macro_frame.take(){self.store_widget_macro_frame(next,frame,closed_instance.is_some())?;}
            Ok((commands,reply))
        })?;
        for command in &mut commands{
            if let crate::eval::UiCommand::Menu{source,..}=command{*source=Some(screen.into());}
            if let crate::eval::UiCommand::Open{host_role,inherit_host:true,..}=command{*host_role=role;}
        }
        Ok((CallbackCommands{commands,owner:owner.clone(),random:random_context},reply))
    }

    pub(super) fn media_handler_owned_budgeted(
        &self,
        next: &mut GameState,
        id: &str,
        handler: &str,
        budget: &mut crate::ui::CanvasBudget,
    ) -> Result<CallbackCommands, RuntimeError> {
        let owner = Self::live_resource(next, crate::scene_resources::ResourceTaskKind::Video, id)
            .map(|task| task.key.owner)
            .unwrap_or_else(|| Self::command_owner(next));
        let track = next
            .videos
            .tracks
            .get(id)
            .ok_or_else(|| self.host_random_error("Video handler has no exact track"))?;
        let projection=self.video_projection_binding(next,id,track)?;
        let mut native_frame=self.native_media_frame(next,handler,projection.as_ref())?;
        let authority =
            if (next.scene_execution.is_some()||next.ui.instance_registry.is_some()) && matches!(owner, TaskOwner::Host { .. }) {
                if let Some(binding) = &projection {
                    Some(self.capture_ui_random_authority(next, &binding.interface)?)
                } else if track.clip.target.is_some() {
                    return Err(self
                        .host_random_error("Embedded Host video has no exact resident instance"));
                } else {
                    None
                }
            } else {
                None
            };
        let event = Value::Dict(BTreeMap::from([
            ("video".into(), Value::Str(id.into())),
            (
                "kind".into(),
                Value::Str(
                    if track.playback == crate::video::Playback::Ended {
                        "end"
                    } else {
                        "error"
                    }
                    .into(),
                ),
            ),
            ("position".into(), Value::Float(track.position as f32)),
            (
                "message".into(),
                Value::Str(track.message.clone().unwrap_or_default()),
            ),
        ]));
        let random_context = authority.map(|authority| Rc::new(RefCell::new(authority)));
        let commands = self.evaluate_owned_random(
            next,
            &owner,
            random_context.as_ref(),
            false,
            |next, random| {
                let (variables, commands) = self
                    .ui_library.with_locale(self.locale.as_ref())
                    .media_event_budgeted_frame(
                        handler,
                        event,
                        &self.functions,
                        &self.global_vars_for_state(next),
                        random,
                        budget,
                        native_frame.as_mut().map(|frame|&mut frame.frame),
                    )
                    .map_err(|error| self.eval_err(error, "Owned video event handler"))?;
                next.vars = Self::without_persistent_globals(variables);
                if let Some(frame)=native_frame.take(){self.store_native_media_frame(next,frame)?;}
                Ok(commands)
            },
        )?;
        Ok(CallbackCommands {
            commands,
            owner,
            random: random_context,
        })
    }

    pub(super) fn evaluate_command_random<T>(
        &self,
        next: &mut GameState,
        owner: &TaskOwner,
        context: Option<&SharedUiRandom>,
        operation: impl FnOnce(&mut GameState, &mut RandomState) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        self.evaluate_owned_random(next, owner, context, true, operation)
    }

    pub(super) fn apply_callback_commands_budgeted(
        &self,
        next: &mut GameState,
        callback: CallbackCommands,
        budget: &mut crate::ui::CanvasBudget,
    ) -> Result<(), RuntimeError> {
        self.apply_ui_commands_budgeted_context(
            next,
            callback.commands,
            budget,
            callback.owner,
            callback.random,
        )
    }
}

// Append to engine_host_random.rs. These exercise actual accepted Engine paths.
#[cfg(test)]
mod host_random_contracts {
    use super::*;
    use crate::engine::{GameVideoFeedback, GameVideoReport, HostFrameReports};
    use crate::renderer::SceneRenderBatch;
    use crate::scene_resources::ResourceTaskKind;
    use rvn_parser::{Hotspot, Position, Transition};

    #[derive(Default)]
    struct Host {
        batches: Vec<SceneRenderBatch>,
        reject: bool,
    }
    impl Renderer for Host {
        fn supports_programmable_ui(&self) -> bool {
            true
        }
        fn supports_video(&self) -> bool {
            true
        }
        fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
            if self.reject {
                Err("Host RNG publication refused".into())
            } else {
                Ok(())
            }
        }
        fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
            self.batches.push(batch)
        }
        fn set_background(&mut self, _: &str, _: &Transition) {}
        fn show_sprite(
            &mut self,
            _: &str,
            _: Option<&str>,
            _: &Position,
            _: &Transition,
            _: Option<&SpriteState>,
        ) {
        }
        fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
        fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
        fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
        fn show_choice(&mut self, _: &[String]) -> usize {
            0
        }
        fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
            0
        }
        fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
        fn music_stop(&mut self, _: &Transition) {}
        fn music_set_volume(&mut self, _: f32) {}
        fn sfx_play(&mut self, _: &str, _: &Transition) {}
        fn sfx_stop(&mut self, _: &str, _: &Transition) {}
    }
    const SOURCE: &str = r#"
screen host(ctx) {return component("launch","button",{"events":{"click":"launch_auto"}},[])}
handler launch_auto(event) {video.play("auto",video_clip("clip.webm",{"on_end":"auto_end"}))}
handler auto_end(event) {set auto_roll=random(1,100) set auto_calls=auto_calls+1 video.play("auto",video_clip("clip.webm",{"on_end":"auto_end"}))}
init {set auto_roll=0 set auto_calls=0}
scene label P
"parent"
"returned"
scene label C
"child"
return
"#;
    fn prepared(source: &str) -> Engine<Host> {
        let mut game = Engine::from_program(
            rvn_parser::parse_program(source).unwrap(),
            Host::default(),
            32,
        )
        .unwrap();
        game.host_random.seed = RandomState::seeded(17);
        game.step_until_interaction().unwrap();
        let context = crate::ui::value_from_json(&rvn_ui::source_menus::preview_context(
            rvn_ui::PageRole::Pause,
            true,
        ))
        .unwrap();
        game.synchronize_source_menu(rvn_ui::PageRole::Pause, Some("host"), context, true, 500)
            .unwrap();
        game.interface_event(crate::ui::UiInput {
            screen: "host".into(),
            element: "launch".into(),
            kind: rvn_ui::programmable::ScreenEventKind::Click,
            value: None,
            key: None,
        })
        .unwrap();
        game.synchronize_source_menu(
            rvn_ui::PageRole::Pause,
            None,
            Value::Dict(BTreeMap::new()),
            true,
            500,
        )
        .unwrap();
        assert!(game.state.ui.screens.is_empty());
        game
    }
    fn end(game: &Engine<Host>) -> GameVideoReport {
        GameVideoReport {
            token: game
                .scene_resource_token(ResourceTaskKind::Video, "auto")
                .unwrap(),
            feedback: GameVideoFeedback::Report(crate::video::Feedback::End),
        }
    }
    fn exact(game: &Engine<Host>) -> serde_json::Value {
        serde_json::json!({"state":game.state,"history":game.history.entries().iter()
            .map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),
            "runtime":game.runtime_epoch(),"interface":game.interface_epoch()})
    }

    #[test]
    fn autonomous_host_rng_persists_between_real_media_events_without_an_interface_or_story_draw() {
        let mut game = prepared(SOURCE);
        let story_random = game.state.random;
        let mut expected = game
            .host_random
            .clone()
            .stream(game.runtime_epoch(), rvn_ui::PageRole::Pause.id())
            .unwrap();
        for calls in [1, 2] {
            let roll = expected.integer(1, 100).unwrap();
            let report = end(&game);
            game.tick_host_frame_with_reports(
                0.0,
                HostFrameReports {
                    videos: vec![report],
                },
            )
            .unwrap();
            assert_eq!(game.state.vars["auto_roll"], Value::Int(roll));
            assert_eq!(game.state.vars["auto_calls"], Value::Int(calls));
            assert_eq!(
                game.host_random.streams[rvn_ui::PageRole::Pause.id()],
                expected
            );
            assert_eq!(game.state.random, story_random);
            assert!(game.state.ui.screens.is_empty());
        }
    }

    #[test]
    fn autonomous_host_rng_and_all_core_fields_commit_only_after_renderer_acceptance() {
        let mut game = prepared(SOURCE);
        let before = exact(&game);
        let rng = game.host_random.clone();
        let batches = game.renderer.batches.len();
        let report = end(&game);
        game.renderer.reject = true;
        assert!(game
            .tick_host_frame_with_reports(
                0.0,
                HostFrameReports {
                    videos: vec![report.clone()]
                }
            )
            .is_err());
        assert_eq!(exact(&game), before);
        assert_eq!(game.host_random, rng);
        assert_eq!(game.renderer.batches.len(), batches);
        let mut expected = rng
            .clone()
            .stream(game.runtime_epoch(), rvn_ui::PageRole::Pause.id())
            .unwrap();
        let roll = expected.integer(1, 100).unwrap();
        game.renderer.reject = false;
        game.tick_host_frame_with_reports(
            0.0,
            HostFrameReports {
                videos: vec![report],
            },
        )
        .unwrap();
        assert_eq!(game.state.vars["auto_roll"], Value::Int(roll));
        assert_eq!(
            game.host_random.streams[rvn_ui::PageRole::Pause.id()],
            expected
        );
        assert_eq!(game.renderer.batches.len(), batches + 1);
    }

    #[test]
    fn autonomous_host_rng_handler_error_and_shared_budget_refusal_are_retry_neutral() {
        let source = SOURCE.replace(
            "set auto_calls=auto_calls+1",
            "set auto_calls=auto_calls+1 set bad=1/0",
        );
        let mut game = prepared(&source);
        let before = exact(&game);
        let rng = game.host_random.clone();
        let batches = game.renderer.batches.len();
        let report = end(&game);
        for _ in 0..2 {
            assert!(game
                .tick_host_frame_with_reports(
                    0.0,
                    HostFrameReports {
                        videos: vec![report.clone()]
                    }
                )
                .is_err());
            assert_eq!(exact(&game), before);
            assert_eq!(game.host_random, rng);
            assert_eq!(game.renderer.batches.len(), batches);
        }
        let source = SOURCE.replace(
            "set auto_calls=auto_calls+1",
            "set auto_calls=auto_calls+1 local i=0 while i<15000 {set i=i+1}",
        );
        let mut game = prepared(&source);
        let before = exact(&game);
        let rng = game.host_random.clone();
        let report = end(&game);
        let batches = game.renderer.batches.len();
        let error = game
            .tick_host_frame_with_reports(
                0.0,
                HostFrameReports {
                    videos: vec![report],
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("100 000 opérations"), "{error}");
        assert_eq!(exact(&game), before);
        assert_eq!(game.host_random, rng);
        assert_eq!(game.renderer.batches.len(), batches);
    }

    #[test]
    fn autonomous_host_rng_is_absent_from_portable_save_and_does_not_continue_in_a_fresh_engine() {
        let mut game = prepared(SOURCE);
        let report = end(&game);
        game.tick_host_frame_with_reports(
            0.0,
            HostFrameReports {
                videos: vec![report],
            },
        )
        .unwrap();
        assert!(!game.host_random.streams.is_empty());
        let save =
            crate::save::SaveData::try_from_state(&game.state, 1, "Host".into(), "main.rvn".into())
                .unwrap();
        let wire = serde_json::to_value(&save).unwrap();
        assert!(!wire.to_string().contains("host_random"));
        let old = game
            .scene_resource_token(ResourceTaskKind::Video, "auto")
            .unwrap();
        let before = exact(&game);
        let random = game.host_random.clone();
        let batches = game.renderer.batches.len();
        game.renderer.reject = true;
        assert!(game.load_data(save.clone()).is_err());
        assert_eq!(exact(&game), before);
        assert_eq!(game.host_random, random);
        assert_eq!(game.renderer.batches.len(), batches);
        game.renderer.reject = false;
        game.load_data(save.clone()).unwrap();
        assert!(game.host_random.streams.is_empty());
        assert_eq!(game.host_random.epoch, None);
        assert_eq!(
            game.video_report_authority(&old).unwrap(),
            crate::engine::VideoReportAuthority::Stale
        );
        assert!(game
            .state
            .videos
            .tracks
            .values()
            .all(|track| track.clip.source != "clip.webm"));
        let fresh = game.fresh(Host::default(), 32).unwrap();
        assert!(fresh.host_random.streams.is_empty());
        assert_eq!(fresh.host_random.epoch, None);
        assert_ne!(fresh.runtime_epoch(), game.runtime_epoch());
        assert!(!fresh.state.videos.tracks.contains_key("auto"));
        assert_eq!(
            fresh.video_report_authority(&old).unwrap(),
            crate::engine::VideoReportAuthority::Stale
        );
        assert_eq!(serde_json::to_value(save).unwrap(), wire);
    }
}

#[cfg(test)]
mod composition_budget_contracts {
    use super::*;
    use crate::engine::{GameVideoFeedback,GameVideoReport,HostFrameReports};
    use crate::renderer::SceneRenderBatch;
    use crate::scene_resources::ResourceTaskKind;
    use rvn_parser::{Hotspot,Position,Transition};
#[derive(Default)]
struct Host {
    batches: Vec<SceneRenderBatch>,
    reject: bool,
}
impl Renderer for Host {
    fn supports_video(&self) -> bool {
        true
    }
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn supports_layered_characters(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("complete Host media candidate rejected".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.batches.push(batch);
    }
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(
        &mut self,
        _: &str,
        _: Option<&str>,
        _: &Position,
        _: &Transition,
        _: Option<&SpriteState>,
    ) {
    }
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}


    const SOURCE: &str=r#"
function pick_face(attributes) {set roll=random(1,100) set i=0 while i<SELECTOR_LOOPS {set i=i+1} if roll<=50 {return {"face":"a"}} return {"face":"b"}}
function portrait() {return layered_image([64,64],{"face":"a"},image_layers("avatar",["avatar__face__a.png","avatar__face__b.png"]),{"selector":"pick_face"})}
screen host(ctx) {return component("launch","button",{"events":{"click":"launch_auto"}},[])}
handler launch_auto(event) {video.play("auto",video_clip("clip.webm",{"on_end":"auto_end"}))}
handler auto_end(event) {set auto_roll=random(1,100) set auto_calls=auto_calls+1 character.compose("avatar",portrait()) character.attributes("avatar",{"face":"a"})}
init {set auto_roll=0 set auto_calls=0}
scene label P
"parent"
scene label C
"child"
return
"#;
    fn prepared(loops:usize)->Engine<Host>{
        let mut game=Engine::from_program(rvn_parser::parse_program(&SOURCE.replace("SELECTOR_LOOPS",&loops.to_string())).unwrap(),Host::default(),32).unwrap();
        game.host_random.seed=RandomState::seeded(17);game.step_until_interaction().unwrap();
        let context=crate::ui::value_from_json(&rvn_ui::source_menus::preview_context(rvn_ui::PageRole::Pause,true)).unwrap();
        game.synchronize_source_menu(rvn_ui::PageRole::Pause,Some("host"),context,true,500).unwrap();
        game.interface_event(crate::ui::UiInput{screen:"host".into(),element:"launch".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}).unwrap();
        game.synchronize_source_menu(rvn_ui::PageRole::Pause,None,Value::Dict(BTreeMap::new()),true,500).unwrap();
        assert!(game.state.ui.screens.is_empty());game
    }
    fn report(game:&Engine<Host>)->GameVideoReport{GameVideoReport{token:game.scene_resource_token(ResourceTaskKind::Video,"auto").unwrap(),feedback:GameVideoFeedback::Report(crate::video::Feedback::End)}}
    fn exact(game:&Engine<Host>)->serde_json::Value{serde_json::json!({"state":game.state,"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),"runtime":game.runtime_epoch(),"interface":game.interface_epoch()})}
    #[test]
    fn autonomous_host_selector_aggregate_refusal_keeps_volatile_rng_token_and_whole_candidate(){
        let mut game=prepared(7500);let before=exact(&game);let rng=game.host_random.clone();let batches=game.renderer.batches.len();let report=report(&game);
        let error=game.tick_host_frame_with_reports(0.0,HostFrameReports{videos:vec![report.clone()]}).unwrap_err();
        assert!(error.to_string().contains("100 000 opérations"),"{error}");
        assert_eq!(exact(&game),before);assert_eq!(game.host_random,rng);assert_eq!(game.renderer.batches.len(),batches);
        assert_eq!(game.video_report_authority(&report.token).unwrap(),crate::engine::VideoReportAuthority::Current(crate::scene_tasks::TaskOwner::Host{role:rvn_ui::PageRole::Pause.id().into()}));
        assert!(game.state.ui.screens.is_empty());assert!(game.state.layered.characters.is_empty());
    }
    #[test]
    fn accepted_autonomous_selectors_draw_once_after_renderer_refusal_and_retry(){
        let mut game=prepared(0);let before=exact(&game);let rng=game.host_random.clone();let batches=game.renderer.batches.len();let report=report(&game);let story_rng=game.state.random;
        let mut expected=rng.clone().stream(game.runtime_epoch(),rvn_ui::PageRole::Pause.id()).unwrap();
        let roll=expected.integer(1,100).unwrap();let _compose=expected.integer(1,100).unwrap();let attributes=expected.integer(1,100).unwrap();
        game.renderer.reject=true;
        assert!(game.tick_host_frame_with_reports(0.0,HostFrameReports{videos:vec![report.clone()]}).is_err());
        assert_eq!(exact(&game),before);assert_eq!(game.host_random,rng);assert_eq!(game.renderer.batches.len(),batches);
        game.renderer.reject=false;game.tick_host_frame_with_reports(0.0,HostFrameReports{videos:vec![report]}).unwrap();
        assert_eq!(game.state.vars["auto_roll"],Value::Int(roll));assert_eq!(game.state.vars["auto_calls"],Value::Int(1));
        assert_eq!(game.state.layered.characters["avatar"].attributes["face"],if attributes<=50 {"a"}else{"b"});
        assert_eq!(game.host_random.streams[rvn_ui::PageRole::Pause.id()],expected);assert_eq!(game.state.random,story_rng);
        assert_eq!(game.renderer.batches.len(),batches+1);assert!(game.state.ui.screens.is_empty());
    }
}
