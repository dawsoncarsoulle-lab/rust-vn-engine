//! Typed portable VM roots. Cursor JSON is decoded only by its actual kind;
//! arbitrary user dictionaries are never searched for magic reference fields.
use super::*;
use crate::widget_reference_values_0974::{ReferencePhase, WidgetValueAdmission};

impl PortableRecord {
    pub(crate) fn visit_value_roots(
        &self,
        visit: &mut impl FnMut(&Value) -> Result<(), String>,
    ) -> Result<(), String> {
        super::super::widget_values::request_value_roots(&self.request, visit)?;
        super::super::widget_values::state_value_roots(&self.result.state, visit)?;
        for value in self.result.variables.values() {
            visit(value)?;
        }
        if let Some(value) = &self.result.value {
            visit(value)?;
        }
        Ok(())
    }
    pub(crate) fn visit_value_roots_mut(
        &mut self,
        visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
    ) -> Result<(), String> {
        super::super::widget_values::request_values(&mut self.request, visit)?;
        super::super::widget_values::state_values(&mut self.result.state, visit)?;
        for value in self.result.variables.values_mut() {
            visit(value)?;
        }
        if let Some(value) = &mut self.result.value {
            visit(value)?;
        }
        Ok(())
    }
}
impl PortableTrace {
    pub(crate) fn visit_value_roots_mut(
        &mut self,
        visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
    ) -> Result<(), String> {
        for record in &mut self.records {
            record.visit_value_roots_mut(visit)?;
        }
        super::super::widget_values::request_values(&mut self.pending.request, visit)?;
        super::super::widget_values::state_values(&mut self.pending.state, visit)?;
        for frame in &mut self.pending.frames {
            super::super::widget_values::frame_values(frame, visit)?;
        }
        for value in &mut self.pending.values {
            visit(value)?;
        }
        Ok(())
    }
    pub(crate) fn export_widget_values(
        &mut self,
        admission: &WidgetValueAdmission,
    ) -> Result<(), String> {
        self.visit_value_roots_mut(&mut |value| admission.value(value, ReferencePhase::Export))?;
        // The actual Continuation's live Tasks were validated before this
        // wire projection. Only this capture path may null a stale cursor link.
        visit_cursor_values_mut(&mut self.pending.tasks, &mut |value| {
            admission.value(value, ReferencePhase::ExportCursor)
        })?;
        self.widget_values = None;
        Ok(())
    }
    pub(crate) fn admit_widget_values(
        &mut self,
        admission: std::sync::Arc<WidgetValueAdmission>,
    ) -> Result<(), String> {
        self.visit_value_roots_mut(&mut |value| admission.value(value, ReferencePhase::Hydrate))?;
        // Only typed cursor payloads are projected; user dictionaries retain
        // their own Value::Dict boundary and cannot masquerade as a reference.
        visit_cursor_values_mut(&mut self.pending.tasks, &mut |value| {
            admission.value(value, ReferencePhase::Portable)
        })?;
        self.widget_values = Some(admission);
        Ok(())
    }
}

fn visit_cursor_values_mut(
    tasks: &mut [TaskCursor],
    visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    for task in tasks {
        match task.kind.as_str() {
            "each_next" => {
                if task
                    .data
                    .as_array()
                    .is_none_or(|values| values.len() > crate::value_limits::MAX_COLLECTION_ITEMS)
                {
                    return Err(
                        "Saved Widget loop cursor exceeds its actual value cardinality".into(),
                    );
                }
                let mut values: Vec<Value> = serde_json::from_value(std::mem::take(&mut task.data))
                    .map_err(|error| error.to_string())?;
                for value in &mut values {
                    visit(value)?;
                }
                task.data = serde_json::to_value(values).map_err(|error| error.to_string())?;
            }
            "consumer_choice" | "consumer_choice_selected" | "consumer_choice_label" => {
                if task
                    .data
                    .as_array()
                    .and_then(|items| items.get(1))
                    .and_then(serde_json::Value::as_array)
                    .is_none_or(|values| values.len() > crate::value_limits::MAX_COLLECTION_ITEMS)
                {
                    return Err(
                        "Saved Widget choice cursor exceeds its actual value cardinality".into(),
                    );
                }
                let (index, mut values): (usize, Vec<Value>) =
                    serde_json::from_value(std::mem::take(&mut task.data))
                        .map_err(|error| error.to_string())?;
                for value in &mut values {
                    visit(value)?;
                }
                task.data =
                    serde_json::to_value((index, values)).map_err(|error| error.to_string())?;
            }
            _ => {}
        }
    }
    Ok(())
}
