//! Reached Scene effects share the existing object admission and backend port.
//! No committing public factory/viewport call is nested inside a consumer.
use super::*;
use crate::widget_backend_0956::{PreparedWidgetViewport, WidgetViewportOperation};
use std::sync::Arc;

pub(super) enum ViewportAction { Add { z_order: i32, modal: bool }, Remove }
impl<R: Renderer> Engine<R> {
    /// End only the current presentation capability, retaining the actual
    /// UObject-equivalent handle/target and all independent resource clocks.
    /// renew_task already owns revision overflow and waiter preservation.
    fn revoke_widget_viewport_input(&self,next:&mut GameState,handle:&WidgetInstanceHandle)->Result<(),RuntimeError>{
        let resident=crate::ui_residency::resident_interface(next,&handle.target)
            .map_err(|message|self.scene_resource_error(message))?;
        let key=crate::scene_resources::resource_key(resident.owner.clone(),crate::scene_resources::ResourceTaskKind::Interface,&resident.identity.alias);
        let registry=if let Some(execution)=&mut next.scene_execution{&mut execution.resources}
            else{next.ui.instance_registry.as_mut().ok_or_else(||self.scene_resource_error("Widget viewport lost its actual instance registry".into()))?};
        if !registry.renew_task(&key).map_err(|message|self.scene_resource_error(message))?{
            return Err(self.scene_resource_error("Widget viewport lost its exact interface task".into()));
        }
        Ok(())
    }
    /// The callback VM has completed in its disposable candidate before this
    /// command is applied. Keep the exact logical instance resident, revoke
    /// only its presentation, and let the existing scene batch plan retain
    /// the original wrapper/source for Destruct and one atomic publication.
    pub(in crate::engine) fn stage_callback_widget_removal(
        &self, next: &mut GameState, object: &Value,
    ) -> Result<(), RuntimeError> {
        let handle = self.widget_reference_handle(object)?;
        if !self.widget_instance(next, &handle)?.viewport_attached() { return Ok(()); }
        self.revoke_widget_viewport_input(next,&handle)?;
        let instance = crate::ui_consumer::instance_mut(next, &handle.target)
            .map_err(|message| self.scene_resource_error(message))?;
        let binding = instance.widget_class.as_mut()
            .ok_or_else(|| self.scene_resource_error("Widget callback lost its actual class".into()))?;
        binding.viewport_attached = false;
        binding.pending_materialization = false;
        instance.focus = None;
        Ok(())
    }
    // Used by both the existing public API and the new Scene command. The
    // caller owns the ordinary host-random transaction and final publication.
    pub(super) fn stage_widget_viewport(
        &self, next: &mut GameState, handle: &WidgetInstanceHandle, action: ViewportAction,
    ) -> Result<Option<Arc<dyn PreparedWidgetViewport>>, RuntimeError> {
        let attached = self.widget_instance(next, handle)?.viewport_attached();
        let (operation, attachment) = match action {
            ViewportAction::Add { z_order, modal } => {
                if attached { return Err(self.scene_resource_error("Widget instance is already attached to the viewport".into())); }
                if !self.renderer.supports_programmable_ui() { return Err(self.scene_resource_error("Renderer cannot host a UserWidget viewport owner".into())); }
                (WidgetViewportOperation::Attach, Some((z_order, modal)))
            }
            ViewportAction::Remove => {
                if !attached { return Ok(None); }
                let pending = self.widget_instance(next, handle)?.widget_class.as_ref().is_some_and(|binding| binding.pending_materialization);
                (if pending { WidgetViewportOperation::Destroy } else { WidgetViewportOperation::Detach }, None)
            }
        };
        let prepared = self.prepare_widget_backend(handle, operation)?;
        if let Some((z_order, modal)) = attachment {
            let instance = crate::ui_consumer::instance_mut(next, &handle.target).map_err(|message| self.scene_resource_error(message))?;
            instance.widget_class.as_mut().ok_or_else(|| self.scene_resource_error("Widget class binding disappeared".into()))?.viewport_attached = true;
            instance.modal = modal;
            instance.layer = z_order;
            if prepared.constructs_wrapper() {
                self.widget_owner_callbacks(next, handle, &[rvn_ui::programmable::HostEventKind::PreConstruct, rvn_ui::programmable::HostEventKind::Construct])?;
            }
        } else {
            if prepared.releases_wrapper() {
                let object = prepared.released_object().ok_or_else(|| self.scene_resource_error("Widget backend release lost its exact wrapper generation".into()))?;
                self.record_widget_retirement(&object)?;
                self.widget_owner_callbacks(next, handle, &[rvn_ui::programmable::HostEventKind::Destruct])?;
            }
            self.revoke_widget_viewport_input(next,handle)?;
            let instance = crate::ui_consumer::instance_mut(next, &handle.target).map_err(|message| self.scene_resource_error(message))?;
            let class = instance.widget_class.as_mut().ok_or_else(|| self.scene_resource_error("Widget class binding disappeared".into()))?;
            class.viewport_attached = false;
            class.pending_materialization = false;
            instance.focus = None;
        }
        Ok(Some(prepared))
    }

    pub(in crate::engine) fn exec_widget_viewport_consumer(&mut self, statement: Statement) -> Result<(), RuntimeError> {
        let Statement::ScheduledConsumer { version: 2, body, .. } = &statement else {
            return Err(self.scene_resource_error("Widget viewport effect requires its actual v2 Scene Consumer".into()));
        };
        let [leaf] = body.as_slice() else { return Err(self.scene_resource_error("Widget viewport effect requires one direct instruction".into())); };
        let input = rvn_parser::widget_viewport_effect_1003::parts(leaf).ok_or_else(|| self.scene_resource_error("Widget viewport instruction differs".into()))?;
        if self.initializing_consumers || self.consumer_program.source.is_none() || self.narrative.is_none()
            || self.state.scene_execution.as_ref().and_then(|execution| execution.active.as_ref()).is_none() {
            return Err(self.scene_resource_error("Widget viewport effects require the actually reached source-owned Scene; Init/Handler/Screen are not installed".into()));
        }
        let (mut next, values) = self.gather_scheduled_action_inputs(&statement)?;
        let Value::List(values) = values else { return Err(self.scene_resource_error("Widget viewport gather lost its actual operand tuple".into())); };
        let (object, action) = match (input, values.as_slice()) {
            (rvn_parser::widget_viewport_effect_1003::ViewportInput::Add { .. }, [object, Value::Int(z_order)]) => {
                let z_order = i32::try_from(*z_order).map_err(|_| self.scene_resource_error("Widget ZOrder requires an exact int32; no truncation".into()))?;
                (object, ViewportAction::Add { z_order, modal: false })
            }
            (rvn_parser::widget_viewport_effect_1003::ViewportInput::Remove { .. }, [object]) => (object, ViewportAction::Remove),
            _ => return Err(self.scene_resource_error("Widget viewport operands require WidgetObject and exact integer ZOrder".into())),
        };
        // This existing method validates typed null, class receipt, live runtime
        // epoch and exact resident target incarnation, never an alias lookup.
        let handle = self.widget_reference_handle(object)?;
        let prepared = self.stage_widget_viewport(&mut next, &handle, action)?;
        next.pc = next.pc.checked_add(1).ok_or_else(|| self.eval_err(EvalError::NumericOverflow, "Widget viewport PC"))?;
        match prepared {
            Some(prepared) => self.commit_widget_candidate(next, prepared),
            None => self.commit_scheduled_state(next),
        }
    }
}
