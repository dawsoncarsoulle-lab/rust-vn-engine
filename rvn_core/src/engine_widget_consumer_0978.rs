//! One reached Scene Consumer stages a factory result in its existing candidate.
//! There is no result map, callback Engine pointer or nested public factory call.
use super::*;
use rvn_parser::widget_create_effect::{ClassInput, FactoryInput};
impl<R: Renderer> Engine<R> {
    pub(super) fn exec_widget_factory_consumer(
        &mut self,
        statement: Statement,
    ) -> Result<(), RuntimeError> {
        let Statement::ScheduledConsumer {
            version: 2, body, ..
        } = &statement
        else {
            return Err(self.scene_resource_error(
                "Widget factory requires its actual v2 Scene Consumer".into(),
            ));
        };
        let [leaf] = body.as_slice() else {
            return Err(
                self.scene_resource_error("Widget factory lost its direct instruction".into())
            );
        };
        let input = rvn_parser::widget_create_effect::parts(leaf)
            .map_err(|error| self.scene_resource_error(error))?
            .ok_or_else(|| {
                self.scene_resource_error("Widget factory lost its actual assignment".into())
            })?;
        // This first port is a real Scene path. Init/Handler need their retained
        // operation/result protocol and must not silently use this fallback.
        if self.initializing_consumers
            || self.consumer_program.source.is_none()
            || self.narrative.is_none()
            || self
                .state
                .scene_execution
                .as_ref()
                .and_then(|e| e.active.as_ref())
                .is_none()
        {
            return Err(self.scene_resource_error("Widget factory currently requires an active source-owned Scene; Init/Handler result continuation is not installed".into()));
        }
        if let ClassInput::Asset(uuid) = input.class {
            if self.widget_classes.class(uuid).is_none() {
                return Err(self.scene_resource_error(
                    "Widget class is not admitted by the actual loader".into(),
                ));
            }
        }
        let (mut next, values) = self.gather_scheduled_action_inputs(&statement)?;
        let Value::List(mut ports) = values else {
            return Err(self.scene_resource_error("Widget factory lost its gathered tuple".into()));
        };
        let expected = if matches!(input.class, ClassInput::Value(_)) {
            2
        } else {
            1
        };
        if ports.len() != expected {
            return Err(self
                .scene_resource_error("Widget factory gathered a different operand count".into()));
        }
        let arguments = ports.pop().expect("exact operand count");
        let Value::List(arguments) = arguments else {
            return Err(self.scene_resource_error("Widget factory arguments must be a list".into()));
        };
        if arguments.len() > 128 {
            return Err(
                self.scene_resource_error("Widget factory argument count exceeds128".into())
            );
        }
        for value in &arguments {
            crate::value_limits::value_work(value)
                .map_err(|error| self.eval_err(error, "Widget factory argument"))?;
        }
        let class = match input.class {
            ClassInput::Asset(uuid) => Some(uuid.to_owned()),
            ClassInput::Null => {
                if !arguments.is_empty() {
                    return Err(self.scene_resource_error(
                        "An explicit null class cannot accept spawn arguments".into(),
                    ));
                }
                None
            }
            ClassInput::Value(_) => {
                let Value::WidgetClass(class) = ports.pop().expect("exact operand count") else {
                    return Err(self.scene_resource_error("Widget factory requires a typed WidgetClass value; no implicit class conversion".into()));
                };
                match class.as_ref() {
                    None => {
                        if !arguments.is_empty() {
                            return Err(self.scene_resource_error("An explicit null class cannot accept spawn arguments".into()));
                        }
                        None
                    },
                    Some(class) => {
                        let actual = self
                            .widget_classes
                            .class(class.uuid())
                            .ok_or_else(|| {
                                self.scene_resource_error(
                                    "Widget class value has no actual admitted source".into(),
                                )
                            })?
                            .identity();
                        if class.receipt() != &actual.receipt {
                            return Err(self.scene_resource_error(
                                "Widget class value differs from its actual source receipt".into(),
                            ));
                        }
                        Some(class.uuid().to_owned())
                    },
                }
            }
        };
        let result = if let Some(uuid) = class {
            // Source names never become object identity. Use existing next_order
            // with a bounded collision check, consumed only in the same next.
            let alias = (0..=32)
                .map(|suffix| format!("__rvn_widget_{:016x}_{suffix}", next.ui.next_order))
                .find(|alias| !next.ui.screens.iter().any(|screen| screen.name == *alias))
                .ok_or_else(|| {
                    self.scene_resource_error(
                        "Widget factory has no bounded free internal address".into(),
                    )
                })?;
            let target = self.prepare_widget_creation_in(&mut next, &uuid, &alias, arguments)?;
            self.widget_reference_value_in_candidate(&next, &target)?
        } else {
            Value::Widget(Box::new(
                rvn_parser::widget_reference::WidgetReference::null(),
            ))
        };
        self.install_widget_factory_destination(&mut next, &input, result)?;
        next.pc = next
            .pc
            .checked_add(1)
            .ok_or_else(|| self.eval_err(EvalError::NumericOverflow, "Widget factory pc"))?;
        self.commit_scheduled_state(next)
    }
    fn install_widget_factory_destination(
        &self,
        next: &mut GameState,
        input: &FactoryInput<'_>,
        value: Value,
    ) -> Result<(), RuntimeError> {
        crate::value_limits::value_work(&value)
            .map_err(|error| self.eval_err(error, "Widget factory result"))?;
        let execution = next.scene_execution.as_mut().ok_or_else(|| {
            self.scene_resource_error("Widget factory lost its Scene candidate".into())
        })?;
        if input.local {
            execution
                .active
                .as_mut()
                .ok_or_else(|| {
                    self.scene_resource_error(
                        "Widget factory local lost its actual activation".into(),
                    )
                })?
                .locals
                .insert(input.binding.into(), value);
        } else if self.narrative.as_ref().is_some_and(|program| {
            program
                .temporary_writes
                .get(&self.state.pc)
                .is_some_and(|name| name == input.binding)
        }) {
            execution
                .temporaries_mut()
                .insert(input.binding.into(), value);
        } else if execution
            .active
            .as_ref()
            .is_some_and(|active| active.locals.contains_key(input.binding))
        {
            execution
                .active
                .as_mut()
                .expect("actual local owner")
                .locals
                .insert(input.binding.into(), value);
        } else {
            next.vars.insert(input.binding.into(), value);
        }
        Ok(())
    }
    fn widget_reference_value_in_candidate(
        &self,
        next: &GameState,
        target: &crate::scene_resources::TargetIdentity,
    ) -> Result<Value, RuntimeError> {
        self.widget_classes
            .validate_state(next)
            .map_err(|error| self.scene_resource_error(error))?;
        let resident = crate::ui_residency::resident_interface(next, target)
            .map_err(|error| self.scene_resource_error(error))?;
        let class = &resident
            .instance
            .widget_class
            .as_ref()
            .ok_or_else(|| {
                self.scene_resource_error("Widget candidate result has no actual class".into())
            })?
            .class;
        let class = rvn_parser::widget_reference::WidgetClassReference::new(
            class.uuid.clone(),
            class.receipt,
        )
        .map_err(|error| self.scene_resource_error(error))?;
        let logical = rvn_parser::widget_reference::PortableWidgetReference::new(
            class,
            target.alias.clone(),
            target.incarnation,
        )
        .map_err(|error| self.scene_resource_error(error))?;
        let reference = rvn_parser::widget_reference::WidgetReference::bind_admitted(
            logical,
            self.instance_id(),
            self.runtime_epoch(),
        )
        .map_err(|error| self.scene_resource_error(error))?;
        Ok(Value::Widget(Box::new(reference)))
    }
}
