//! Value visitation at the existing owned VM/portable boundary.
use super::*;
use crate::widget_reference_values_0974::{ReferencePhase, WidgetValueAdmission};

pub(super) fn state_value_roots(
    state: &State,
    visit: &mut impl FnMut(&Value) -> Result<(), String>,
) -> Result<(), String> {
    for value in state.globals.values() {
        visit(value)?;
    }
    for command in &state.commands {
        match command {
            UiCommand::Open { arguments, .. } => {
                for value in arguments {
                    visit(value)?;
                }
            }
            UiCommand::SetState { state, .. } => visit(state)?,
            UiCommand::WidgetRemoveFromParent { object } => visit(object)?,
            _ => {}
        }
    }
    if let Some(active) = &state.active_consumer {
        for value in active.captures.values() {
            visit(value)?;
        }
    }
    Ok(())
}
pub(super) fn request_value_roots(
    request: &Request,
    visit: &mut impl FnMut(&Value) -> Result<(), String>,
) -> Result<(), String> {
    for value in request.variables.values() {
        visit(value)?;
    }
    if let Root::Call(_, values)|Root::LibraryCall(_, values) = &request.root {
        for value in values {
            visit(value)?;
        }
    }
    state_value_roots(&request.state, visit)
}

fn command_values(
    command: &mut UiCommand,
    visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    match command {
        UiCommand::Open { arguments, .. } => {
            for value in arguments {
                visit(value)?;
            }
        }
        UiCommand::SetState { state, .. } => visit(state)?,
            UiCommand::WidgetRemoveFromParent { object } => visit(object)?,
        _ => {}
    }
    Ok(())
}
pub(super) fn state_values(
    state: &mut State,
    visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    for value in state.globals.values_mut() {
        visit(value)?;
    }
    for command in &mut state.commands {
        command_values(command, visit)?;
    }
    if let Some(active) = &mut state.active_consumer {
        for value in active.captures.values_mut() {
            visit(value)?;
        }
    }
    Ok(())
}
pub(super) fn request_values(
    request: &mut Request,
    visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    for value in request.variables.values_mut() {
        visit(value)?;
    }
    if let Root::Call(_, values)|Root::LibraryCall(_, values) = &mut request.root {
        for value in values {
            visit(value)?;
        }
    }
    state_values(&mut request.state, visit)
}
pub(super) fn frame_values(
    frame: &mut Frame,
    visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    for value in frame.variables.values_mut() {
        visit(value)?;
    }
    if let Some(active) = &mut frame.active_consumer {
        for value in active.captures.values_mut() {
            visit(value)?;
        }
    }
    Ok(())
}
fn task_values(
    task: &mut Task,
    visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    match task {
        Task::EachNext { items, .. } => {
            for value in items.as_mut_slice() {
                visit(value)?;
            }
        }
        Task::ConsumerChoice { rows, .. }
        | Task::ConsumerChoiceSelected { rows, .. }
        | Task::ConsumerChoiceLabel { rows, .. } => {
            for value in rows {
                visit(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
impl Continuation {
    pub(crate) fn visit_value_roots_mut(
        &mut self,
        visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
    ) -> Result<(), String> {
        request_values(&mut self.request, visit)?;
        state_values(&mut self.state, visit)?;
        for frame in &mut self.frames {
            frame_values(frame, visit)?;
        }
        for value in &mut self.values {
            visit(value)?;
        }
        if let Some(value) = &mut self.result {
            visit(value)?;
        }
        for task in &mut self.tasks {
            task_values(task, visit)?;
        }
        Ok(())
    }
    /// Read the actual stopped evaluator without cloning its retained payload.
    pub(crate) fn validate_widget_values(
        &self,
        admission: &WidgetValueAdmission,
    ) -> EvalResult<()> {
        let visit = |value: &Value| {
            admission
                .validate_value(value, ReferencePhase::Live)
                .map_err(|error| invalid(&error))
        };
        fn command(
            command: &UiCommand,
            visit: &impl Fn(&Value) -> EvalResult<()>,
        ) -> EvalResult<()> {
            match command {
                UiCommand::Open { arguments, .. } => {
                    for value in arguments {
                        visit(value)?;
                    }
                }
                UiCommand::SetState { state, .. } => visit(state)?,
            UiCommand::WidgetRemoveFromParent { object } => visit(object)?,
                _ => {}
            }
            Ok(())
        }
        fn state(state: &State, visit: &impl Fn(&Value) -> EvalResult<()>) -> EvalResult<()> {
            for value in state.globals.values() {
                visit(value)?;
            }
            for value in &state.commands {
                command(value, visit)?;
            }
            if let Some(active) = &state.active_consumer {
                for value in active.captures.values() {
                    visit(value)?;
                }
            }
            Ok(())
        }
        for value in self.request.variables.values() {
            visit(value)?;
        }
        if let Root::Call(_, args)|Root::LibraryCall(_, args) = &self.request.root {
            for value in args {
                visit(value)?;
            }
        }
        state(&self.request.state, &visit)?;
        state(&self.state, &visit)?;
        for frame in &self.frames {
            for value in frame.variables.values() {
                visit(value)?;
            }
            if let Some(active) = &frame.active_consumer {
                for value in active.captures.values() {
                    visit(value)?;
                }
            }
        }
        for value in &self.values {
            visit(value)?;
        }
        if let Some(value) = &self.result {
            visit(value)?;
        }
        for task in &self.tasks {
            match task {
                Task::EachNext { items, .. } => {
                    for value in items.as_slice() {
                        visit(value)?;
                    }
                }
                Task::ConsumerChoice { rows, .. }
                | Task::ConsumerChoiceSelected { rows, .. }
                | Task::ConsumerChoiceLabel { rows, .. } => {
                    for value in rows {
                        visit(value)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub(super) fn rebind_task_values(
    tasks: &mut [Task],
    admission: &WidgetValueAdmission,
) -> EvalResult<()> {
    for task in tasks {
        task_values(task, &mut |value| {
            admission.value(value, ReferencePhase::Hydrate)
        })
        .map_err(|error| invalid(&error))?;
    }
    Ok(())
}
