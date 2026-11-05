//! Scoped reference admission on the actual class/residency graph.
//! This is a disposable validation plan, not an object registry or input grant.
use crate::{
    scene_resources::{TargetIdentity, TargetKind},
    widget_class_source_0950::AdmittedWidgetClasses,
    GameState,
};
use rvn_parser::{
    widget_reference::{PortableWidgetReference, WidgetClassReference, WidgetReference},
    Value,
};

#[path="widget_object_class_1002.rs"]
pub(crate) mod object_class;

#[derive(Clone)]
pub(crate) struct WidgetValueAdmission {
    classes: std::sync::Arc<AdmittedWidgetClasses>,
    targets: Vec<PortableWidgetReference>,
    instance: u64,
    epoch: u64,
}
impl std::fmt::Debug for WidgetValueAdmission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WidgetValueAdmission")
            .field("targets", &self.targets.len())
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ReferencePhase {
    Live,
    Export,
    ExportCursor,
    Portable,
    Hydrate,
    RebaseHistory,
}
impl WidgetValueAdmission {
    pub(crate) fn new(
        state: &GameState,
        classes: std::sync::Arc<AdmittedWidgetClasses>,
        instance: u64,
        epoch: u64,
    ) -> Result<Self, String> {
        classes.validate_state(state)?;
        let mut plan = Self {
            classes: classes.clone(),
            targets: Vec::new(),
            instance,
            epoch,
        };
        if let Some(registry) = state
            .scene_execution
            .as_ref()
            .map(|execution| &execution.resources)
            .or(state.ui.instance_registry.as_ref())
        {
            for target in registry
                .targets
                .iter()
                .filter(|record| record.identity.kind == TargetKind::Interface)
            {
                let resident = crate::ui_residency::resident_interface(state, &target.identity)?;
                let Some(binding) = &resident.instance.widget_class else {
                    continue;
                };
                let identity = classes
                    .class(&binding.class.uuid)
                    .ok_or("Widget value class is not admitted")?
                    .identity();
                if binding.class != *identity {
                    return Err("Widget value class/source differs from actual admission".into());
                }
                let class = WidgetClassReference::new(identity.uuid.clone(), identity.receipt)?;
                let reference = PortableWidgetReference::new(
                    class,
                    target.identity.alias.clone(),
                    target.identity.incarnation,
                )?;
                if plan.targets.contains(&reference) {
                    return Err("Widget value target is ambiguous".into());
                }
                plan.targets.push(reference);
            }
        }
        Ok(plan)
    }
    pub(crate) fn reference(
        &self,
        portable: &PortableWidgetReference,
    ) -> Result<WidgetReference, String> {
        portable.validate()?;
        if !self.targets.contains(portable) {
            return Err(
                "Widget value does not link its exact admitted resident class/target".into(),
            );
        }
        WidgetReference::bind_admitted(portable.clone(), self.instance, self.epoch)
    }
    pub(crate) fn target(&self, value: &WidgetReference) -> Result<TargetIdentity, String> {
        let portable = value.portable().ok_or("Widget reference is null")?;
        if !value.is_bound_to(self.instance, self.epoch) {
            return Err("Widget value belongs to another runtime or is still portable data".into());
        }
        self.reference(portable)?;
        Ok(TargetIdentity {
            kind: TargetKind::Interface,
            alias: portable.alias().into(),
            incarnation: portable.incarnation(),
        })
    }
    pub(crate) fn validate_value(
        &self,
        value: &Value,
        phase: ReferencePhase,
    ) -> Result<(), String> {
        crate::value_limits::value_work(value).map_err(|error| error.to_string())?;
        fn walk(
            plan: &WidgetValueAdmission,
            value: &Value,
            phase: ReferencePhase,
        ) -> Result<(), String> {
            match value {
                Value::WidgetClass(class) => {
                    let Some(class) = class.as_ref() else { return Ok(()); };
                    class.validate()?;
                    if plan
                        .classes
                        .class(class.uuid())
                        .is_none_or(|actual| actual.identity().receipt != *class.receipt())
                    {
                        return Err(
                            "Widget class value differs from admitted source receipt".into()
                        );
                    }
                }
                Value::Widget(reference) => {
                    let Some(portable) = reference.portable() else {
                        return Ok(());
                    };
                    portable.validate()?;
                    let class = portable.class();
                    if plan
                        .classes
                        .class(class.uuid())
                        .is_none_or(|actual| actual.identity().receipt != *class.receipt())
                    {
                        return Err("Widget reference class/source receipt is not admitted".into());
                    }
                    match phase {
                        ReferencePhase::Live | ReferencePhase::Export if !reference.is_bound_to(plan.instance, plan.epoch) => return Err("Widget value has stale or portable runtime provenance".into()),
                        ReferencePhase::RebaseHistory if !reference.was_bound_to_instance(plan.instance) => return Err("Historical Widget reference belongs to another Engine or is inert data".into()),
                        ReferencePhase::Portable | ReferencePhase::Hydrate | ReferencePhase::ExportCursor if !reference.is_inert() => return Err("Portable Widget value contains a live runtime binding".into()),
                        _ => {}
                    }
                    if matches!(phase, ReferencePhase::Portable | ReferencePhase::Hydrate)
                        && !plan.targets.contains(portable)
                    {
                        return Err(
                            "Portable Widget reference does not link its exact saved class/target"
                                .into(),
                        );
                    }
                }
                Value::List(items) => {
                    for item in items {
                        walk(plan, item, phase)?;
                    }
                }
                Value::Dict(items) => {
                    for item in items.values() {
                        walk(plan, item, phase)?;
                    }
                }
                _ => {}
            }
            Ok(())
        }
        walk(self, value, phase)
    }
    pub(crate) fn value(&self, value: &mut Value, phase: ReferencePhase) -> Result<(), String> {
        self.validate_value(value, phase)?;
        fn transform(
            plan: &WidgetValueAdmission,
            value: &mut Value,
            phase: ReferencePhase,
        ) -> Result<(), String> {
            match value {
                Value::Widget(reference) => {
                    if let Some(portable) = reference.portable() {
                        if matches!(
                            phase,
                            ReferencePhase::Export
                                | ReferencePhase::ExportCursor
                                | ReferencePhase::RebaseHistory
                        ) && !plan.targets.contains(portable)
                        {
                            **reference = WidgetReference::null();
                        } else if matches!(
                            phase,
                            ReferencePhase::Hydrate | ReferencePhase::RebaseHistory
                        ) {
                            **reference = plan.reference(portable)?;
                        } else if matches!(
                            phase,
                            ReferencePhase::Export | ReferencePhase::ExportCursor
                        ) {
                            **reference = reference.inert_data();
                        }
                    }
                }
                Value::List(items) => {
                    for value in items {
                        transform(plan, value, phase)?;
                    }
                }
                Value::Dict(items) => {
                    for value in items.values_mut() {
                        transform(plan, value, phase)?;
                    }
                }
                _ => {}
            }
            Ok(())
        }
        transform(self, value, phase)
    }
}

pub(crate) fn visit_instance_values_mut(
    instance: &mut crate::ui::ScreenInstance,
    visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    for value in &mut instance.arguments {
        visit(value)?;
    }
    for value in instance.values.values_mut() {
        visit(value)?;
    }
    for canvas in instance.canvas_states.values_mut() {
        visit(&mut canvas.state)?;
    }
    Ok(())
}

pub(crate) fn visit_state_values_mut(
    state: &mut GameState,
    visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    fn ui(
        ui: &mut crate::ui::UiState,
        visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
    ) -> Result<(), String> {
        for instance in &mut ui.screens {
            visit_instance_values_mut(instance, visit)?;
        }
        Ok(())
    }
    fn activation(
        active: &mut crate::scene::SceneActivation,
        visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
    ) -> Result<(), String> {
        for value in active
            .locals
            .values_mut()
            .chain(active.temporaries.values_mut())
        {
            visit(value)?;
        }
        ui(&mut active.tasks.suspended_ui, visit)?;
        Ok(())
    }
    fn dialogue(
        dialogue: &mut crate::types::DialogueSnapshot,
        visit: &mut impl FnMut(&mut Value) -> Result<(), String>,
    ) -> Result<(), String> {
        for value in dialogue.vars.values_mut() {
            visit(value)?;
        }
        if let Some(globals) = &mut dialogue.lexical_globals {
            for value in globals.values_mut() {
                visit(value)?;
            }
        }
        if let Some(inputs) = &mut dialogue.consumer_dialogue {
            for value in &mut inputs.values {
                visit(value)?;
            }
        }
        Ok(())
    }
    for value in state.vars.values_mut() {
        visit(value)?;
    }
    if let Some(dialogue_value) = &mut state.last_dialogue {
        dialogue(dialogue_value, visit)?;
    }
    if let Some(inputs) = &mut state.consumer_dialogue {
        for value in &mut inputs.values {
            visit(value)?;
        }
    }
    if let Some(choice) = &mut state.consumer_choice {
        for row in &mut choice.rows {
            for value in &mut row.values {
                visit(value)?;
            }
        }
    }
    ui(&mut state.ui, visit)?;
    if let Some(execution) = &mut state.scene_execution {
        for value in execution.pre_scene_temporaries.values_mut() {
            visit(value)?;
        }
        ui(&mut execution.game_tasks.suspended_ui, visit)?;
        if let Some(active) = &mut execution.active {
            activation(active, visit)?;
        }
        for frame in &mut execution.frames {
            if let Some(active) = &mut frame.parked {
                activation(active, visit)?;
            }
            if let Some(values) = &mut frame.pre_scene_temporaries {
                for value in values.values_mut() {
                    visit(value)?;
                }
            }
            if let Some(tasks) = &mut frame.pre_scene_resources {
                ui(&mut tasks.suspended_ui, visit)?;
            }
        }
        if let Some(display) = &mut execution.display {
            for value in display
                .variables
                .values_mut()
                .chain(display.lexical_globals.values_mut())
            {
                visit(value)?;
            }
            if let Some(inputs) = &mut display.consumer_dialogue {
                for value in &mut inputs.values {
                    visit(value)?;
                }
            }
            if let Some(choice) = &mut display.consumer_choice {
                for row in &mut choice.rows {
                    for value in &mut row.values {
                        visit(value)?;
                    }
                }
            }
        }
        execution.visit_native_loop_frames_mut(|frame| {
            for record in &mut frame.gather {
                record.visit_value_roots_mut(visit)?;
            }
            Ok::<_, String>(())
        })?;
    }
    Ok(())
}

pub(crate) fn reject_widget_values(value: &Value) -> Result<(), String> {
    crate::value_limits::value_work(value).map_err(|error| error.to_string())?;
    fn reject(value: &Value) -> Result<(), String> {
        match value {
            Value::Widget(_) | Value::WidgetClass(_)=>return Err("Widget references require the admitted Save15 adapter and cannot enter historical saves or persistent data".into()),
            Value::List(items)=>for value in items { reject(value)?; },
            Value::Dict(items)=>for value in items.values() { reject(value)?; },
            _=>{}
        }
        Ok(())
    }
    reject(value)
}
