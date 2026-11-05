//! Child of eval: macro control is executed in the existing Computation and
//! shares its steps/value work/RNG/return/break semantics. No nested evaluator.
use super::*;
use crate::widget_class_source_0950::standard_macros::{Operand, OperandNeed};
use rvn_parser::scheduled_source::standard_macro_contract::{DataOutput, Dispatch, Site};
fn invalid(reason: String) -> EvalError {
    EvalError::InvalidFunction(reason)
}
impl Computation<'_> {
    pub(super) fn macro_dispatch(
        &mut self,
        dispatch: &Dispatch,
        variables: &mut HashMap<String, Value>,
    ) -> EvalResult<Option<Value>> {
        dispatch.validate().map_err(invalid)?;
        if self.calls != 0 || self.handler_locals.is_none() || self.pure_canvas {
            return Err(invalid(
                "Stateful macro requires its actual Widget Handler execution".into(),
            ));
        }
        let frame = self.widget_macro_frame.as_ref().ok_or_else(|| {
            invalid("Macro Handler has no authenticated Widget instance owner".into())
        })?;
        let site = self
            .library
            .widget_macro_sites
            .site(frame.class(), &dispatch.site)
            .map_err(invalid)?
            .clone();
        let need = frame
            .operand_need(&site, dispatch.control)
            .map_err(invalid)?;
        let operand = match need {
            OperandNeed::None => Operand::None,
            OperandNeed::StartClosed => {
                let value = self.deferred_macro_value(dispatch, variables)?;
                let Value::Bool(value) = value else {
                    return Err(invalid("StartClosed must retain its Boolean type".into()));
                };
                Operand::StartClosed(value)
            }
            OperandNeed::Limit => {
                let value = self.deferred_macro_value(dispatch, variables)?;
                let Value::Int(value) = value else {
                    return Err(invalid("DoN N must retain its signed int32 type".into()));
                };
                Operand::Limit(
                    crate::standard_macro_state::DoNLimit::try_from(value)
                        .map_err(|_| invalid("DoN N is outside signed int32".into()))?,
                )
            }
        };
        let before = self
            .widget_macro_frame
            .as_ref()
            .unwrap()
            .retained_bytes()
            .map_err(invalid)?;
        let emitted = self
            .widget_macro_frame
            .as_mut()
            .unwrap()
            .step(&site, dispatch.control, operand)
            .map_err(invalid)?;
        let after = self
            .widget_macro_frame
            .as_ref()
            .unwrap()
            .retained_bytes()
            .map_err(invalid)?;
        self.value_work = self
            .value_work
            .checked_add(
                after
                    .checked_sub(before)
                    .ok_or(EvalError::NumericOverflow)?,
            )
            .ok_or(EvalError::NumericOverflow)?;
        if self.value_work > 1_000_000 {
            return Err(EvalError::ExecutionLimit {
                limit: "1 000 000 copies ou caractères de valeurs par calcul",
            });
        }
        if let Some(output) = emitted {
            // The cell write above is visible to any recipient reentry/live
            // Counter/IsA read before this branch body is evaluated.
            let branch = dispatch
                .branches
                .iter()
                .find(|branch| branch.output == output)
                .ok_or_else(|| invalid("Emitted macro output lost its actual branch".into()))?;
            self.block(&branch.body, variables)
        } else {
            Ok(None)
        }
    }
    pub(super) fn macro_data(
        &self,
        declaration: &Site,
        output: DataOutput,
        pin: u64,
    ) -> EvalResult<Value> {
        if self.pure_canvas {
            return Err(invalid(
                "Pure projection cannot borrow mutable Widget Handler macro ownership".into(),
            ));
        }
        let frame = self
            .widget_macro_frame
            .as_ref()
            .ok_or_else(|| invalid("Macro data read has no authenticated Widget owner".into()))?;
        let site = self
            .library
            .widget_macro_sites
            .site(frame.class(), declaration)
            .map_err(invalid)?;
        frame.data(site, output, pin).map_err(invalid)
    }
}

impl FunctionLibrary {
    /// One existing evaluator for both independently owned frame families.
    /// Writes are returned only after successful execution; the Engine's real
    /// candidate still controls command/backend publication and abandonment.
    pub(crate) fn handle_event_result_in_instance_frames_budgeted_with_locale(
        &self,
        parameters: &[String],
        body: &[Statement],
        arguments: &[Value],
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
        budget: &mut crate::ui::CanvasBudget,
        locale: Option<&crate::locale::LocaleManager>,
        native: Option<&mut crate::consumer_schedule::ConsumerFrame>,
        macros: Option<&mut crate::widget_class_source_0950::standard_macros::WidgetMacroFrame>,
    ) -> EvalResult<(HashMap<String, Value>, Vec<UiCommand>, Option<Value>)> {
        validate_handler(body)?;
        if parameters.len() != arguments.len() {
            return Err(invalid("Handler argument count differs".into()));
        }
        if native.is_none() && crate::ui::contains_native_plan(body) {
            return Err(invalid(
                "Native Handler requires its actual retained instance frame".into(),
            ));
        }
        let storage = match native.as_deref() {
            Some(frame) => frame.storage()?,
            None => Default::default(),
        };
        let macro_bytes = match macros.as_deref() {
            Some(frame) => {
                self.widget_macro_sites
                    .validate_frame(frame)
                    .map_err(invalid)?;
                frame.retained_bytes().map_err(invalid)?
            }
            None => 0,
        };
        let mut locals = globals.clone();
        locals.extend(parameters.iter().cloned().zip(arguments.iter().cloned()));
        let mut computation = Computation::new(self, globals);
        computation.locale = locale;
        computation.steps = budget.steps;
        computation.value_work = budget
            .value_work
            .checked_add(storage.bytes)
            .and_then(|n| n.checked_add(macro_bytes))
            .ok_or(EvalError::NumericOverflow)?;
        computation.handler_locals = Some(parameters.iter().cloned().collect());
        computation.random = *random;
        if let Some(frame) = native.as_deref() {
            computation.consumer_frame = frame.clone();
        }
        computation.consumer_storage = storage;
        computation.widget_macro_frame = macros.as_deref().cloned();
        let result = computation.block(body, &mut locals);
        budget.steps = computation.steps;
        budget.value_work = computation.value_work;
        let returned = result?;
        *random = computation.random;
        if let Some(frame) = native {
            *frame = computation.consumer_frame;
        }
        if let Some(frame) = macros {
            *frame = computation
                .widget_macro_frame
                .ok_or_else(|| invalid("Widget macro owner disappeared during Handler".into()))?;
        }
        Ok((computation.globals, computation.ui_commands, returned))
    }
}
