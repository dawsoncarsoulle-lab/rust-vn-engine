//! Child of host_consumer, reusing its exact resident/closed-interface routing.
use super::*;
use crate::widget_class_source_0950::standard_macros::WidgetMacroFrame;
impl<R: Renderer> Engine<R> {
    pub(in crate::engine) fn widget_macro_handler_frame(
        &self,
        next: &GameState,
        screen: &str,
        handler: &str,
        owner: &crate::scene_tasks::TaskOwner,
        closed: Option<&ClosedInterface>,
    ) -> Result<Option<WidgetMacroFrame>, RuntimeError> {
        let (source, target) = self.routed_handler_instance(next, screen, owner, closed)?;
        self.widget_classes
            .validate_owner_handler(source.widget_class.as_ref(), handler)
            .map_err(|message| self.scene_resource_error(message))?;
        let Some(binding) = source.widget_class.as_ref() else {
            return Ok(None);
        };
        if !self
            .functions
            .widget_macro_sites
            .class_has_handler(&binding.class, handler)
        {
            return Ok(None);
        }
        let target = target.ok_or_else(|| {
            self.scene_resource_error("Macro requires the actual Widget incarnation".into())
        })?;
        let frame = WidgetMacroFrame::owned(
            self.instance_id(),
            target,
            binding,
            source.standard_macro_frame.as_ref(),
            &self.widget_classes,
        )
        .map_err(|message| self.scene_resource_error(message))?;
        self.functions
            .widget_macro_sites
            .validate_frame(&frame)
            .map_err(|message| self.scene_resource_error(message))?;
        Ok(Some(frame))
    }
    pub(in crate::engine) fn store_widget_macro_frame(
        &self,
        next: &mut GameState,
        frame: WidgetMacroFrame,
        closed: bool,
    ) -> Result<(), RuntimeError> {
        self.functions
            .widget_macro_sites
            .validate_frame(&frame)
            .map_err(|message| self.scene_resource_error(message))?;
        if frame.lineage() != self.instance_id() {
            return Err(self.scene_resource_error(
                "Macro candidate belongs to another actual Engine namespace".into(),
            ));
        }
        // A closed-instance callback can consume its disposable captured cells;
        // it never writes them into a replacement object with the same alias.
        if closed {
            return Ok(());
        }
        let resident = crate::ui_residency::resident_interface(next, frame.target())
            .map_err(|s| self.scene_resource_error(s))?;
        if resident.residence != crate::ui_residency::ProjectionResidence::Visible
            || resident
                .instance
                .widget_class
                .as_ref()
                .is_none_or(|binding| &binding.class != frame.class())
        {
            return Err(self.scene_resource_error(
                "Macro candidate lost its exact visible Widget source".into(),
            ));
        }
        let order = resident.instance.order;
        let target = frame.target().clone();
        let instance = next
            .ui
            .screens
            .iter_mut()
            .find(|i| i.name == target.alias && i.order == order)
            .ok_or_else(|| {
                self.scene_resource_error(
                    "Macro instance disappeared before candidate write".into(),
                )
            })?;
        instance.standard_macro_frame = if frame.is_empty() { None } else { Some(frame) };
        Ok(())
    }
}

impl<R: Renderer> Engine<R> {
    /// Exact-context observation port for the existing inspector/debugger.
    /// Existing WidgetInstanceHandle validation checks Engine/epoch/lifetime;
    /// the inspector does not mint an owner from a graph id or display alias.
    pub fn inspect_widget_macro_cell(
        &self,
        handle: &WidgetInstanceHandle,
        graph: u64,
        node: u64,
    ) -> Result<
        (
            crate::program::GraphInstructionOrigin,
            crate::widget_class_source_0950::standard_macros::CellInspection,
        ),
        RuntimeError,
    > {
        let owner = self.widget_backend_owner(handle)?;
        let resident = crate::ui_residency::resident_interface(&self.state, owner.target())
            .map_err(|s| self.scene_resource_error(s))?;
        let binding = resident.instance.widget_class.as_ref().ok_or_else(|| {
            self.scene_resource_error("Macro inspection has no actual Widget class".into())
        })?;
        let frame = WidgetMacroFrame::owned(
            self.instance_id(),
            resident.identity,
            binding,
            resident.instance.standard_macro_frame.as_ref(),
            &self.widget_classes,
        )
        .map_err(|s| self.scene_resource_error(s))?;
        self.functions
            .widget_macro_sites
            .validate_frame(&frame)
            .map_err(|s| self.scene_resource_error(s))?;
        let site = self
            .functions
            .widget_macro_sites
            .by_identity(&binding.class, graph, node)
            .map_err(|s| self.scene_resource_error(s))?;
        Ok((
            site.origin().clone(),
            frame
                .inspect(site)
                .map_err(|s| self.scene_resource_error(s))?,
        ))
    }
}
