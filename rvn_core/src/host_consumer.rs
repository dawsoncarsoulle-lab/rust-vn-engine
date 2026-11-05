//! Engine child module: exact UI lifetime capabilities never come from a DTO.
use super::*;
#[path="engine_widget_macros_1020.rs"]
mod standard_macros_1020;
use crate::consumer_schedule::HostDefinitionKind;
use crate::scene_resources::TargetIdentity;
use crate::ui_consumer::{owned_frame, BoundInstanceConsumerFrame, InstanceProjection};

#[derive(Clone)]
pub(super) struct ClosedInterface {
    target: Option<TargetIdentity>,
    owner: crate::scene_tasks::TaskOwner,
    instance: crate::ui::ScreenInstance,
}
impl ClosedInterface{
    pub(super) fn owner_instance(&self,name:&str,owner:&crate::scene_tasks::TaskOwner)->Result<&crate::ui::ScreenInstance,&'static str>{
        if self.instance.name!=name||self.owner!=*owner{return Err("Owner callback differs from actual closed instance");}
        Ok(&self.instance)
    }
}
impl<R: Renderer> Engine<R> {
    pub(super) fn portable_host_input(
        &self,
        input: &crate::ui_input::ScopedUiInput,
    ) -> Result<crate::save::consumer_checkpoint::PortableHostInput, String> {
        use rvn_ui::programmable::ScreenEventKind;
        // Save15 validates an already stopped, authentic logical continuation.
        // This never admits an external producer while its wrapper is pending.
        if !crate::ui_input::current_logical(&self.state,self.instance_id(),self.runtime_epoch(),self.interface_epoch(),&input.source) || input.source.screen != input.event.screen
        {
            return Err("Stopped input no longer belongs to its real source".into());
        }
        let token =
            match &input.source.authority {
                crate::ui_input::UiInputAuthority::Narrative(token)
                | crate::ui_input::UiInputAuthority::Instance { token, .. } => token,
                _ => return Err(
                    "Native UI continuation needs a real instance registry, not legacy alias/epoch"
                        .into(),
                ),
            };
        let event = &input.event;
        if event.screen.len() > 128
            || event.element.len() > 128
            || event.key.as_ref().is_some_and(|key| key.len() > 256)
            || matches!(
                event.kind,
                ScreenEventKind::Open | ScreenEventKind::Close | ScreenEventKind::Tick
            )
        {
            return Err("Invalid portable external UI event".into());
        }
        if let Some(value) = &event.value {
            crate::value_limits::value_work(value).map_err(|e| e.to_string())?;
        }
        self.consumer_program
            .validate_emitted(&self.script)
            .map_err(|e| e.to_string())?;
        let resident = crate::ui_residency::resident_interface(&self.state, &token.target)?;
        if resident.residence != crate::ui_residency::ProjectionResidence::Visible {
            return Err("Stopped input instance is parked".into());
        }
        let views = self.describe_uncached_interfaces(&self.state).map_err(|e| e.to_string())?;
        let view = views
            .iter()
            .find(|view| view.name == event.screen)
            .ok_or("Stopped input view is absent")?;
        if event.kind != ScreenEventKind::PointerCancel
            && (crate::ui::top_modal_owner(&self.state.ui)?.is_some_and(|modal|(view.layer,view.order)<modal)
                || !resident.instance.owner_properties.allows_child_event(event.kind)
                || !view.root.available(&event.element))
        {
            return Err("Stopped input component is unavailable".into());
        }
        let component = view
            .root
            .find(&event.element)
            .ok_or("Stopped input component is absent")?;
        let handler = self
            .ui_library
            .event_handler(event, component)
            .ok_or("Stopped input has no actual routed handler")?;
        self.widget_classes.validate_owner_handler(resident.instance.widget_class.as_ref(),handler)?;
        let definition = self
            .consumer_program
            .host_definition(&HostDefinitionKind::Handler, handler)
            .filter(|d| d.native_plans && d.kind == HostDefinitionKind::Handler)
            .ok_or("Stopped input has no frozen native handler")?;
        let screen = self
            .consumer_program
            .host_definition(&HostDefinitionKind::Screen, resident.instance.definition())
            .filter(|d| d.kind == HostDefinitionKind::Screen)
            .ok_or("Stopped input has no frozen Screen definition")?;
        Ok(crate::save::consumer_checkpoint::PortableHostInput {
            target: resident.identity,
            screen_ordinal: screen.source_ordinal,
            handler: handler.into(),
            handler_ordinal: definition.source_ordinal,
            screen: event.screen.clone(),
            element: event.element.clone(),
            kind: event.kind,
            value: event.value.clone(),
            key: event.key.clone(),
        })
    }
    /// Reachable only from the actual restored DebugSession whose private
    /// materialization barrier, source anchor and complete trace were admitted.
    /// Public Input/debug_begin still use strict presentation admission.
    pub(super) fn resume_restored_host_input(
        &mut self, input: crate::ui_input::ScopedUiInput,
    ) -> Result<bool, RuntimeError> {
        if !self.restored_widget_barrier {
            return Err(self.scene_resource_error("Saved Host input has no materialization boundary".into()));
        }
        if !crate::ui_input::current_logical(&self.state,self.instance_id(),self.runtime_epoch(),self.interface_epoch(),&input.source)
            || input.source.screen!=input.event.screen { return Ok(false); }
        self.with_host_random_transaction(|engine|engine.interface_event_host_planned(input.event))?;
        Ok(true)
    }
    pub(super) fn restore_host_input(
        &self,
        portable: &crate::save::consumer_checkpoint::PortableHostInput,
    ) -> Result<crate::ui_input::ScopedUiInput, String> {
        let source=crate::ui_input::capture_logical(&self.state,self.instance_id(),self.runtime_epoch(),self.interface_epoch(),&portable.screen)
            .ok_or("Portable input has no restored logical registry source")?;
        let token = match &source.authority {
            crate::ui_input::UiInputAuthority::Narrative(token)
            | crate::ui_input::UiInputAuthority::Instance { token, .. } => token,
            _ => return Err("Portable input cannot hydrate a legacy owner".into()),
        };
        if token.target != portable.target {
            return Err("Portable input target/incarnation differs from restored registry".into());
        }
        let input = crate::ui_input::ScopedUiInput {
            source,
            event: crate::ui::UiInput {
                screen: portable.screen.clone(),
                element: portable.element.clone(),
                kind: portable.kind,
                value: portable.value.clone(),
                key: portable.key.clone(),
            },
        };
        let actual = self.portable_host_input(&input)?;
        if actual.screen_ordinal != portable.screen_ordinal
            || actual.handler != portable.handler
            || actual.handler_ordinal != portable.handler_ordinal
        {
            return Err(
                "Portable input definition/source ordinal differs from frozen routed event".into(),
            );
        }
        Ok(input)
    }
    pub(super) fn host_input_namespaces(
        &self,
        portable: &crate::save::consumer_checkpoint::PortableHostInput,
    ) -> Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
        String,
    > {
        self.consumer_program
            .host_definition(&HostDefinitionKind::Handler, &portable.handler)
            .filter(|d| {
                d.native_plans
                    && d.kind == HostDefinitionKind::Handler
                    && d.source_ordinal == portable.handler_ordinal
            })
            .ok_or("Host namespace has no actual frozen handler")?;
        let binding = self
            .consumer_program
            .source
            .as_ref()
            .ok_or("Host namespace has no frozen source")?;
        let mut roots = std::collections::BTreeMap::new();
        for (root, definition) in &binding.definitions {
            let name = &definition.definition;
            let kind = match definition.kind {
                HostDefinitionKind::Screen => "screen",
                HostDefinitionKind::Handler => "handler",
            };
            let scopes = definition
                .scopes
                .keys()
                .map(|scope| {
                    (
                        scope.clone(),
                        format!(
                            "host-source:{}:{kind}:{}:{name}:{scope}",
                            definition.source_ordinal,
                            name.len()
                        ),
                    )
                })
                .collect();
            roots.insert(root.clone(), scopes);
        }
        Ok(roots)
    }
    pub(super) fn capture_closing_interface(
        &self,
        state: &GameState,
        name: &str,
    ) -> Result<Option<ClosedInterface>, RuntimeError> {
        if !state
            .ui
            .screens
            .iter()
            .any(|instance| instance.name == name)
        {
            return Ok(None);
        }
        if state.scene_execution.is_none() && state.ui.instance_registry.is_none() {
            // Legacy Close still dispatches after removal. Retain its real
            // pre-removal instance/order/owner without inventing registry or
            // Native input authority for an alias which is now absent.
            let instance=state.ui.screens.iter().find(|instance|instance.name==name).ok_or_else(||self.scene_resource_error("Closing legacy instance disappeared".into()))?;
            let owner=instance.host_role.map(|role|crate::scene_tasks::TaskOwner::Host{role:role.id().into()}).unwrap_or_else(||Self::command_owner(state));
            return Ok(Some(ClosedInterface{target:None,owner,instance:instance.clone()}));
        }
        let target = crate::ui_residency::visible_interface_target(state, name)
            .map_err(|message| self.scene_resource_error(message))?;
        let resident = crate::ui_residency::resident_interface(state, &target)
            .map_err(|message| self.scene_resource_error(message))?;
        Ok(Some(ClosedInterface {
            target: Some(resident.identity),
            owner: resident.owner,
            instance: resident.instance.clone(),
        }))
    }
    pub(super) fn native_event_frame(
        &self,next:&GameState,event:&crate::ui::UiInput,component:&rvn_ui::programmable::Component,
        owner:&crate::scene_tasks::TaskOwner,closed:Option<&ClosedInterface>,
    )->Result<Option<BoundInstanceConsumerFrame>,RuntimeError>{
        let Some(handler)=self.ui_library.event_handler(event,component)else{return Ok(None);};
        self.native_named_instance_handler_frame(next,&event.screen,handler,owner,closed)
    }
    /// Same retained real-instance handler authority for child and owner events.
    /// An owner event never needs a fabricated Component or element ID.
    fn routed_handler_instance<'a>(
        &self,next:&'a GameState,screen:&str,owner:&crate::scene_tasks::TaskOwner,closed:Option<&'a ClosedInterface>,
    )->Result<(&'a crate::ui::ScreenInstance,Option<TargetIdentity>),RuntimeError>{
        if let Some(closed)=closed{
            let instance=closed.owner_instance(screen,owner).map_err(|message|self.scene_resource_error(message.into()))?;
            return Ok((instance,closed.target.clone()));
        }
        if next.scene_execution.is_some()||next.ui.instance_registry.is_some(){
            let target=crate::ui_residency::visible_interface_target(next,screen).map_err(|message|self.scene_resource_error(message))?;
            let resident=crate::ui_residency::resident_interface(next,&target).map_err(|message|self.scene_resource_error(message))?;
            if resident.owner!=*owner||resident.residence!=crate::ui_residency::ProjectionResidence::Visible{return Err(self.scene_resource_error("UI handler differs from exact resident lifecycle owner".into()));}
            return Ok((resident.instance,Some(resident.identity)));
        }
        let instance=next.ui.screens.iter().find(|instance|instance.name==screen).ok_or_else(||self.scene_resource_error("UI handler has no actual legacy instance".into()))?;
        let actual=instance.host_role.map(|role|crate::scene_tasks::TaskOwner::Host{role:role.id().into()}).unwrap_or_else(||Self::command_owner(next));
        if actual!=*owner{return Err(self.scene_resource_error("UI handler differs from actual legacy lifecycle owner".into()));}
        Ok((instance,None))
    }
    pub(super) fn native_named_instance_handler_frame(
        &self,next:&GameState,screen:&str,handler:&str,owner:&crate::scene_tasks::TaskOwner,closed:Option<&ClosedInterface>,
    )->Result<Option<BoundInstanceConsumerFrame>,RuntimeError>{
        // Class membership precedes the Native/Legacy split. A legacy Handler
        // is still a method call on this actual class instance, never a grant
        // to execute an unrelated global definition.
        let (source,identity)=self.routed_handler_instance(next,screen,owner,closed)?;
        self.widget_classes.validate_owner_handler(source.widget_class.as_ref(),handler)
            .map_err(|message|self.scene_resource_error(message))?;
        if !self.consumer_program.native_host_definition(&HostDefinitionKind::Handler,handler){return Ok(None);}
        self.consumer_program.validate_emitted(&self.script).map_err(|error|self.eval_err(error,"UI consumer source"))?;
        let identity=identity.ok_or_else(||self.scene_resource_error("Native UI handler has no actual resident target".into()))?;
        let definition = self
            .consumer_program
            .host_definition(&HostDefinitionKind::Handler, handler)
            .filter(|definition| definition.kind == HostDefinitionKind::Handler)
            .ok_or_else(|| {
                self.scene_resource_error("Routed handler has no frozen source definition".into())
            })?;
        let actual = crate::ui_consumer::InstanceConsumerOwner {
            target: identity,
            source_ordinal: definition.source_ordinal,
            definition: handler.into(),
            kind: HostDefinitionKind::Handler,
        };
        owned_frame(&self.consumer_program, &source.consumer_frames, actual)
            .map(Some)
            .map_err(|e| self.eval_err(e, "Native UI frame"))
    }
    pub(super) fn store_native_event_frame(
        &self,
        next: &mut GameState,
        frame: BoundInstanceConsumerFrame,
        closed: bool,
    ) -> Result<(), RuntimeError> {
        self.consumer_program
            .validate_instance_binding(&frame)
            .map_err(|e| self.eval_err(e, "Native UI frame"))?;
        if closed {
            return Ok(());
        }
        let resident = crate::ui_residency::resident_interface(next, &frame.owner.target)
            .map_err(|s| self.scene_resource_error(s))?;
        if resident.residence != crate::ui_residency::ProjectionResidence::Visible {
            return Err(self.scene_resource_error(
                "Native handler instance became parked during evaluation".into(),
            ));
        }
        let order = resident.instance.order;
        let instance = next
            .ui
            .screens
            .iter_mut()
            .find(|instance| instance.name == frame.owner.target.alias && instance.order == order)
            .ok_or_else(|| {
                self.scene_resource_error("Native handler instance disappeared".into())
            })?;
        instance
            .consumer_frames
            .insert(frame.owner.storage_key(), frame);
        Ok(())
    }
    pub(super) fn native_media_frame(
        &self,next:&GameState,handler:&str,projection:Option<&crate::ui_residency::UiProjectionBinding>,
    )->Result<Option<BoundInstanceConsumerFrame>,RuntimeError>{
        let resident=if let Some(projection)=projection{
            let resident=crate::ui_residency::resident_interface(next,&projection.interface).map_err(|message|self.scene_resource_error(message))?;
            if resident.instance.order!=projection.order||resident.instance.host_role!=projection.host_role||resident.residence!=projection.residence{return Err(self.scene_resource_error("Media projection differs from actual resident instance".into()));}
            self.widget_classes.validate_owner_handler(resident.instance.widget_class.as_ref(),handler).map_err(|message|self.scene_resource_error(message))?;
            Some(resident)
        }else{None};
        if !self.consumer_program.native_host_definition(&HostDefinitionKind::Handler,handler){return Ok(None);}
        self.consumer_program.validate_emitted(&self.script).map_err(|error|self.eval_err(error,"Native media source"))?;
        let resident=resident.ok_or_else(||self.scene_resource_error("Native media handler needs the actual captured interface projection; unbound media cannot choose Game or an alias owner".into()))?;
        let definition = self
            .consumer_program
            .host_definition(&HostDefinitionKind::Handler, handler)
            .filter(|definition| definition.kind == HostDefinitionKind::Handler)
            .ok_or_else(|| {
                self.scene_resource_error("Native media handler has no frozen definition".into())
            })?;
        let owner = crate::ui_consumer::InstanceConsumerOwner {
            target: resident.identity,
            source_ordinal: definition.source_ordinal,
            definition: handler.into(),
            kind: HostDefinitionKind::Handler,
        };
        owned_frame(
            &self.consumer_program,
            &resident.instance.consumer_frames,
            owner,
        )
        .map(Some)
        .map_err(|e| self.eval_err(e, "Native media frame"))
    }
    pub(super) fn store_native_media_frame(
        &self,
        next: &mut GameState,
        frame: BoundInstanceConsumerFrame,
    ) -> Result<(), RuntimeError> {
        self.consumer_program
            .validate_instance_binding(&frame)
            .map_err(|e| self.eval_err(e, "Native media frame"))?;
        let instance = crate::ui_consumer::instance_mut(next, &frame.owner.target)
            .map_err(|s| self.scene_resource_error(s))?;
        instance
            .consumer_frames
            .insert(frame.owner.storage_key(), frame);
        Ok(())
    }
    pub(super) fn prepare_native_screen(
        &self,
        next: &mut GameState,
        name: &str,
        budget: &mut crate::ui::CanvasBudget,
    ) -> Result<Option<rvn_ui::programmable::Component>, RuntimeError> {
        self.consumer_program
            .validate_emitted(&self.script)
            .map_err(|e| self.eval_err(e, "UI definition source"))?;
        let target = crate::ui_residency::visible_interface_target(next, name)
            .map_err(|s| self.scene_resource_error(s))?;
        let resident = crate::ui_residency::resident_interface(next, &target)
            .map_err(|s| self.scene_resource_error(s))?;
        let owner = resident.owner.clone();
        let arguments = resident.instance.arguments.clone();
        let definition=resident.instance.definition().to_owned();
        let frame_owner = self
            .consumer_program
            .capture_instance_owner(next, &target, &definition, HostDefinitionKind::Screen)
            .map_err(|e| self.eval_err(e, "Native Screen owner"))?;
        let mut frame = owned_frame(
            &self.consumer_program,
            &resident.instance.consumer_frames,
            frame_owner.clone(),
        )
        .map_err(|e| self.eval_err(e, "Native Screen frame"))?;
        let (parameters, body) = self.ui_library.screen_definition(&definition).ok_or_else(|| {
            self.scene_resource_error("Native Screen definition disappeared".into())
        })?;
        let random_context = if matches!(owner, crate::scene_tasks::TaskOwner::Host { .. }) {
            Some(std::rc::Rc::new(std::cell::RefCell::new(
                self.capture_visible_ui_random(next, name)?,
            )))
        } else {
            None
        };
        self.evaluate_command_random(next, &owner, random_context.as_ref(), |next, random| {
            let construction_frame=self.functions.debug_probe.as_ref().map(|_|frame.clone());
            let mut describe=|| self
                .functions
                .debugger_scope(format!("screen:{definition}"), || {
                    self.functions
                        .describe_screen_in_consumer_frame_with_locale(
                            parameters,
                            body,
                            &arguments,
                            &self.global_vars_for_state(next),
                            random,
                            budget,
                            self.locale.as_ref(),
                            &mut frame.frame,
                        )
                });
            let description=if let (Some(probe),Some(construction_frame))=(&self.functions.debug_probe,&construction_frame){
                crate::eval::debugger_eval::Probe::screen_construction(probe,self,next,construction_frame,parameters,body,&arguments,describe)
                    .map_err(|e|self.eval_err(e,"Native Screen construction"))?
            }else{describe()}.map_err(|e| self.eval_err(e, "Native Screen gather"))?;
            let description = rvn_ui::programmable::parse_screen_description(
                crate::ui::value_to_json(&description)
                    .map_err(|e| self.eval_err(e, "Native Screen description"))?,
            )
            .map_err(|s| self.scene_resource_error(s))?;
            self.consumer_program
                .validate_instance_binding(&frame)
                .map_err(|e| self.eval_err(e, "Native Screen frame"))?;
            let program = self
                .consumer_program
                .identity()
                .map_err(|e| self.eval_err(e, "Native Screen program"))?;
            let instance = next
                .ui
                .screens
                .iter_mut()
                .find(|instance| instance.name == name)
                .ok_or_else(|| {
                    self.scene_resource_error("Native Screen instance disappeared".into())
                })?;
            instance
                .consumer_frames
                .insert(frame.owner.storage_key(), frame.clone());
            instance.consumer_projection = Some(InstanceProjection {
                program,
                owner: frame_owner.clone(),
                root: description.root.clone(),host:description.host,
            });
            Ok(description.root)
        })
    }
    pub(super) fn refresh_native_targets(
        &self,
        next: &mut GameState,
        targets: &[TargetIdentity],
        budget: &mut crate::ui::CanvasBudget,
    ) -> Result<(), RuntimeError> {
        for target in targets {
            if next.ui.screens.iter().any(|instance| {
                instance
                    .consumer_projection
                    .as_ref()
                    .is_some_and(|projection| projection.owner.target == *target)
            }) && crate::ui_residency::resident_interface(next, target).is_ok()
            {
                self.prepare_native_screen(next, &target.alias, budget)?;
            }
        }
        Ok(())
    }
}
