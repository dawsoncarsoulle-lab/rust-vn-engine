//! Child of the real debugger owner. Candidate inspection is an observation
//! of this accepted session's continuation, never a rebound portable DTO.
use super::*;
use crate::widget_class_source_0950::standard_macros::CellInspection;
impl<R: Renderer> Engine<R> {
    pub fn inspect_debug_widget_macro_cell(
        &self,
        session: &DebugSession,
        handle: &WidgetInstanceHandle,
        graph: u64,
        node: u64,
    ) -> Result<Option<(crate::program::GraphInstructionOrigin, CellInspection)>, RuntimeError>
    {
        if !self.accepts_debug_session(session) || session.complete || session.error.is_some() {
            return Err(self.scene_resource_error(
                "Macro inspection requires this exact live debug operation/source generation"
                    .into(),
            ));
        }
        let owner = self.widget_backend_owner(handle)?;
        let Some(frame) = session
            .continuation
            .as_ref()
            .and_then(Continuation::widget_macro_frame)
        else {
            return Ok(None);
        };
        if frame.lineage() != owner.lineage() || frame.target() != owner.target() {
            return Err(self.scene_resource_error(
                "Macro debug object belongs to another actual Widget instance".into(),
            ));
        }
        let resident = crate::ui_residency::resident_interface(&self.state, owner.target())
            .map_err(|s| self.scene_resource_error(s))?;
        let binding = resident.instance.widget_class.as_ref().ok_or_else(|| {
            self.scene_resource_error("Macro debug object lost its actual class".into())
        })?;
        if frame.class() != &binding.class {
            return Err(
                self.scene_resource_error("Macro debug source/class receipt is stale".into())
            );
        }
        self.functions
            .widget_macro_sites
            .validate_frame(frame)
            .map_err(|s| self.scene_resource_error(s))?;
        let site = self
            .functions
            .widget_macro_sites
            .by_identity(&binding.class, graph, node)
            .map_err(|s| self.scene_resource_error(s))?;
        Ok(Some((
            site.origin().clone(),
            frame
                .inspect(site)
                .map_err(|s| self.scene_resource_error(s))?,
        )))
    }
}

impl<R: Renderer> Engine<R> {
    // A cold restore must not rewrite the Engine lineage of an exact hot trace.
    // Completed saves without debugger data still reset transient cells normally.
    pub(super) fn reject_macro_save15_debugger(
        save: &crate::save::consumer_checkpoint::Save15,
    ) -> Result<(), crate::save::SaveError> {
        if save
            .debugger
            .as_ref()
            .and_then(|d| d.trace.as_ref())
            .is_some_and(|trace| trace.has_widget_macro_owner())
        {
            return Err(crate::save::SaveError::Incompatible(
                "Save15 cannot cold-restore a pending Widget macro owner; finish or abandon the live debug operation before saving".into(),
            ));
        }
        Ok(())
    }

    // Read-only test observation through the same actual resident Host permit;
    // this is no GameSave and grants neither a new owner nor a replay commit.
    #[cfg(test)]
    pub(crate) fn actual_widget_macro_trace_for_test(
        &self,
        session: &DebugSession,
        handle: &WidgetInstanceHandle,
    ) -> Result<crate::eval::debugger_eval::PortableTrace, RuntimeError> {
        if !self.accepts_debug_session(session)
            || session.complete
            || session.error.is_some()
            || session.armed_step.is_some()
        {
            return Err(self.scene_resource_error(
                "Macro trace requires this exact stopped live operation".into(),
            ));
        }
        let owner = self.widget_backend_owner(handle)?;
        let continuation = session.continuation.as_ref().ok_or_else(|| {
            self.scene_resource_error("Macro trace needs its actual continuation".into())
        })?;
        let frame = continuation
            .widget_macro_frame()
            .ok_or_else(|| self.scene_resource_error("Macro trace lost its actual owner".into()))?;
        if frame.lineage() != owner.lineage() || frame.target() != owner.target() {
            return Err(
                self.scene_resource_error("Macro trace belongs to another actual owner".into())
            );
        }
        let permit = crate::ui_consumer::HostContinuationPermit::from_resident(
            &self.consumer_program,
            &self.state,
            owner.target(),
        )
        .map_err(|e| self.scene_resource_error(e.to_string()))?;
        Probe::portable_trace_with_host(&session.probe, continuation, Some(&permit))
            .map_err(|e| self.scene_resource_error(e.to_string()))
    }
}

// Negative external-input fixture only: derive every metadata field from the
// real stopped Input. The inert DTO is NOT an accepted capture or owner grant.
#[cfg(test)]
impl<R: Renderer> Engine<R> {
    pub(crate) fn actual_widget_macro_debug_dto_for_test(
        &self,
        program: &rvn_parser::ParsedProgram,
        session: &DebugSession,
        handle: &WidgetInstanceHandle,
    ) -> Result<crate::save::consumer_checkpoint::PortableDebugState, crate::save::SaveError> {
        use crate::save::{
            consumer_checkpoint::{PortableDebugState, PortableStoppedVisit},
            SaveError,
        };
        let mut trace = self
            .actual_widget_macro_trace_for_test(session, handle)
            .map_err(|e| SaveError::Incompatible(e.to_string()))?;
        let DebugOperation::Input(input) = &session.operation else {
            return Err(SaveError::Incompatible(
                "External macro DTO fixture requires the real Input operation".into(),
            ));
        };
        // Data-only negative input: native_plans is reserved for scheduled
        // persistent projections. A macro-only Handler can be genuinely frozen
        // without such a projection. Read its real route/source metadata; never
        // claim portable_host_input or a pending Save15 capture was admitted.
        self.consumer_program
            .validate_emitted(&self.script)
            .map_err(|e| SaveError::Incompatible(e.to_string()))?;
        let owner = self
            .widget_backend_owner(handle)
            .map_err(|e| SaveError::Incompatible(e.to_string()))?;
        if !crate::ui_input::current_logical(
            &self.state,
            self.instance_id(),
            self.runtime_epoch(),
            self.interface_epoch(),
            &input.source,
        ) || input.source.screen != input.event.screen
            || input.event.screen != owner.target().alias
        {
            return Err(SaveError::Incompatible(
                "Macro external DTO lost its real Input/owner".into(),
            ));
        }
        let resident = crate::ui_residency::resident_interface(&self.state, owner.target())
            .map_err(SaveError::Incompatible)?;
        let views = self
            .describe_uncached_interfaces(&self.state)
            .map_err(|e| SaveError::Incompatible(e.to_string()))?;
        let view = views
            .iter()
            .find(|view| view.name == input.event.screen)
            .ok_or_else(|| {
                SaveError::Incompatible("Macro external DTO lost its real view".into())
            })?;
        let component = view.root.find(&input.event.element).ok_or_else(|| {
            SaveError::Incompatible("Macro external DTO lost its real component".into())
        })?;
        let handler = self
            .ui_library
            .event_handler(&input.event, component)
            .ok_or_else(|| {
                SaveError::Incompatible("Macro external DTO lost its actual routed Handler".into())
            })?;
        self.widget_classes
            .validate_owner_handler(resident.instance.widget_class.as_ref(), handler)
            .map_err(SaveError::Incompatible)?;
        if !self
            .functions
            .widget_macro_sites
            .class_has_handler(owner.class(), handler)
        {
            return Err(SaveError::Incompatible(
                "Macro external DTO Handler has no actual class macro site".into(),
            ));
        }
        let kind = crate::consumer_schedule::HostDefinitionKind::Handler;
        let definition = self
            .consumer_program
            .host_definition(&kind, handler)
            .filter(|d| d.kind == kind)
            .ok_or_else(|| {
                SaveError::Incompatible("Macro external DTO lost its frozen Handler".into())
            })?;
        let screen = self
            .consumer_program
            .host_definition(
                &crate::consumer_schedule::HostDefinitionKind::Screen,
                resident.instance.definition(),
            )
            .ok_or_else(|| {
                SaveError::Incompatible("Macro external DTO lost its frozen Screen".into())
            })?;
        let host_input = crate::save::consumer_checkpoint::PortableHostInput {
            target: resident.identity,
            screen_ordinal: screen.source_ordinal,
            handler: handler.into(),
            handler_ordinal: definition.source_ordinal,
            screen: input.event.screen.clone(),
            element: input.event.element.clone(),
            kind: input.event.kind,
            value: input.event.value.clone(),
            key: input.event.key.clone(),
        };
        let (namespaces, mapping) = crate::save::consumer_checkpoint::portable_namespaces(
            program,
            &self.script,
            &self.consumer_program,
        )?;
        let binding = self.consumer_program.source.as_ref().ok_or_else(|| {
            SaveError::Incompatible("Macro external DTO lost frozen source namespaces".into())
        })?;
        let host_namespaces = binding
            .definitions
            .iter()
            .map(|(root, definition)| {
                let name = &definition.definition;
                let kind = match definition.kind {
                    crate::consumer_schedule::HostDefinitionKind::Screen => "screen",
                    crate::consumer_schedule::HostDefinitionKind::Handler => "handler",
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
                (root.clone(), scopes)
            })
            .collect();
        trace
            .remap_owned_namespaces(&mapping, &host_namespaces)
            .map_err(|e| SaveError::Incompatible(e.to_string()))?;
        let mut dto = PortableDebugState {
            startup: None,
            frame_operation: None,
            host_input: Some(host_input),
            widget_materialization: None,
            trace: Some(trace),
            namespaces,
            pc: session.position.pc,
            breakpoints: session.breakpoints.clone(),
            watches: session.watches.clone(),
            entry_stop: session.entry_stop,
            stopped: session
                .stopped_visit
                .as_ref()
                .map(|v| PortableStoppedVisit {
                    entry: v.entry,
                    site: v.site.clone(),
                }),
            last_site: session.last_site.clone(),
            work: session.work,
        };
        let values = crate::widget_reference_values_0974::WidgetValueAdmission::new(
            &self.state,
            self.widget_classes.clone(),
            self.instance_id(),
            self.runtime_epoch(),
        )
        .map_err(SaveError::Incompatible)?;
        dto.export_widget_values(&values)
            .map_err(SaveError::Incompatible)?;
        Ok(dto)
    }
}
