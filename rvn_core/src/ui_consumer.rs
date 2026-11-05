//! Native inspector storage belongs to an authenticated resident interface,
//! and one actual frozen Screen/Handler definition, never to its task clock.
use crate::consumer_schedule::{ConsumerFrame, ConsumerProgram, HostDefinitionKind};
use crate::eval::{EvalError, EvalResult};
use crate::scene_resources::{TargetIdentity, TargetKind};
use crate::ui_residency::{resident_interface, ProjectionResidence};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
fn invalid(reason: &str) -> EvalError {
    EvalError::InvalidFunction(reason.into())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceConsumerOwner {
    pub target: TargetIdentity,
    pub source_ordinal: usize,
    pub definition: String,
    pub kind: HostDefinitionKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundInstanceConsumerFrame {
    pub program: [u8; 32],
    pub owner: InstanceConsumerOwner,
    pub frame: ConsumerFrame,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceProjection {
    pub program: [u8; 32],
    pub owner: InstanceConsumerOwner,
    pub root: Option<rvn_ui::programmable::Component>,
    #[serde(default,skip_serializing_if="rvn_ui::programmable::ScreenHostProperties::is_default")]
    pub host: rvn_ui::programmable::ScreenHostProperties,
}

impl ConsumerProgram {
    pub(crate) fn host_definition(
        &self,
        kind: &HostDefinitionKind,
        name: &str,
    ) -> Option<&crate::consumer_schedule::HostDefinitionSource> {
        self.source.as_ref()?.definition(kind, name)
    }
    pub(crate) fn native_host_definition(&self, kind: &HostDefinitionKind, name: &str) -> bool {
        self.host_definition(kind, name)
            .is_some_and(|definition| definition.native_plans)
    }
    pub(crate) fn capture_instance_owner(
        &self,
        state: &crate::GameState,
        target: &TargetIdentity,
        definition: &str,
        kind: HostDefinitionKind,
    ) -> EvalResult<InstanceConsumerOwner> {
        let resident = resident_interface(state, target).map_err(|s| invalid(&s))?;
        if resident.residence != ProjectionResidence::Visible {
            return Err(invalid(
                "Native UI consumer needs a visible resident instance",
            ));
        }
        if kind == HostDefinitionKind::Screen && resident.instance.definition() != definition {
            return Err(invalid(
                "Screen definition differs from actual resident instance",
            ));
        }
        let source = self
            .host_definition(&kind, definition)
            .filter(|source| source.kind == kind)
            .ok_or_else(|| invalid("Native UI definition has no actual frozen source binding"))?;
        Ok(InstanceConsumerOwner {
            target: resident.identity,
            source_ordinal: source.source_ordinal,
            definition: definition.into(),
            kind,
        })
    }
    pub(crate) fn validate_instance_binding(
        &self,
        bound: &BoundInstanceConsumerFrame,
    ) -> EvalResult<()> {
        if bound.program != self.identity()? || bound.owner.target.kind != TargetKind::Interface {
            return Err(invalid(
                "Native UI frame program/target differs from authenticated instance",
            ));
        }
        let source = self
            .host_definition(&bound.owner.kind, &bound.owner.definition)
            .filter(|source| {
                source.kind == bound.owner.kind
                    && source.source_ordinal == bound.owner.source_ordinal
            })
            .ok_or_else(|| {
                invalid("Native UI frame differs from frozen definition/source ordinal")
            })?;
        bound.frame.validate()?;
        let frame = serde_json::to_value(&bound.frame).map_err(|e| invalid(&e.to_string()))?;
        let scopes = frame
            .get("scopes")
            .and_then(|value| value.as_object())
            .ok_or_else(|| invalid("Native UI frame lacks typed scopes"))?;
        for (scope, terminals) in scopes {
            let producers = source.scopes.get(scope).ok_or_else(|| {
                invalid("Native UI frame scope is absent from its actual definition")
            })?;
            if terminals
                .as_object()
                .is_none_or(|values| values.keys().any(|producer| !producers.contains(producer)))
            {
                return Err(invalid(
                    "Native UI terminal has no inspector in its actual definition",
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn validate_instance_frames(&self, state: &crate::GameState) -> EvalResult<()> {
        if let Some(registry) = &state.ui.instance_registry {
            if state.scene_execution.is_some() {
                return Err(invalid(
                    "Narrative state cannot carry a detached interface registry",
                ));
            }
            registry
                .validate_interface_registry(&state.ui)
                .map_err(|s| invalid(&s))?;
        }
        let mut checked = 0usize;
        let mut validate = |instance: &crate::ui::ScreenInstance| -> EvalResult<()> {
            if instance.consumer_frames.len() > 128 {
                return Err(invalid("Native UI exceeds 128 retained definition frames"));
            }
            for (key, frame) in &instance.consumer_frames {
                checked += 1;
                if checked > 16384 {
                    return Err(invalid(
                        "Native UI exceeds 16384 retained definition frames across visits",
                    ));
                }
                if key != &frame.owner.storage_key() || instance.name != frame.owner.target.alias {
                    return Err(invalid(
                        "Native UI frame is attached to another definition/instance",
                    ));
                }
                self.validate_instance_binding(frame)?;
                let resident =
                    resident_interface(state, &frame.owner.target).map_err(|s| invalid(&s))?;
                if resident.instance.order != instance.order
                    || resident.instance.host_role != instance.host_role
                {
                    return Err(invalid(
                        "Native UI frame snapshot belongs to another resident incarnation",
                    ));
                }
            }
            if let Some(projection) = &instance.consumer_projection {
                if projection.owner.kind != HostDefinitionKind::Screen
                    || projection.owner.definition != instance.definition()
                    || projection.owner.target.alias != instance.name
                    || projection.program != self.identity()?
                {
                    return Err(invalid(
                        "Native UI projection lacks its actual Screen owner",
                    ));
                }
                let source = self
                    .host_definition(&HostDefinitionKind::Screen, instance.definition())
                    .filter(|source| {
                        source.native_plans
                            && source.kind == HostDefinitionKind::Screen
                            && source.source_ordinal == projection.owner.source_ordinal
                    })
                    .ok_or_else(|| {
                        invalid("Native UI projection differs from actual Screen definition")
                    })?;
                let _ = source;
                resident_interface(state, &projection.owner.target).map_err(|s| invalid(&s))?;
                projection.host.validate().map_err(|s| invalid(&s))?;
                if let Some(root) = &projection.root { root.validate().map_err(|s| invalid(&s))?; }
            }
            Ok(())
        };
        for instance in &state.ui.screens {
            validate(instance)?;
        }
        if let Some(execution) = &state.scene_execution {
            for active in execution.active.iter().chain(
                execution
                    .frames
                    .iter()
                    .filter_map(|frame| frame.parked.as_ref()),
            ) {
                for instance in &active.tasks.suspended_ui.screens {
                    validate(instance)?;
                }
            }
        }
        Ok(())
    }
}
impl InstanceConsumerOwner {
    pub fn storage_key(&self) -> String {
        self.kind.qualified(&self.definition)
    }
}
impl BoundInstanceConsumerFrame {
    pub(crate) fn new(program: &ConsumerProgram, owner: InstanceConsumerOwner) -> EvalResult<Self> {
        Ok(Self {
            program: program.identity()?,
            owner,
            frame: Default::default(),
        })
    }
}
pub(crate) fn owned_frame(
    program: &ConsumerProgram,
    frames: &BTreeMap<String, BoundInstanceConsumerFrame>,
    owner: InstanceConsumerOwner,
) -> EvalResult<BoundInstanceConsumerFrame> {
    match frames.get(&owner.storage_key()) {
        Some(frame) => {
            program.validate_instance_binding(frame)?;
            if frame.owner != owner {
                return Err(invalid(
                    "Native UI frame was replaced by another incarnation",
                ));
            }
            Ok(frame.clone())
        }
        None => BoundInstanceConsumerFrame::new(program, owner),
    }
}

pub(crate) fn instance_mut<'a>(
    state: &'a mut crate::GameState,
    target: &TargetIdentity,
) -> Result<&'a mut crate::ui::ScreenInstance, String> {
    let resident = resident_interface(state, target)?;
    let residence = resident.residence;
    let owner = resident.owner;
    let order = resident.instance.order;
    let ui = match residence {
        ProjectionResidence::Visible => &mut state.ui,
        ProjectionResidence::Parked => {
            let crate::scene_tasks::TaskOwner::Scene { activation } = owner else {
                return Err("Parked interface lacks its actual Scene owner".into());
            };
            let execution = state
                .scene_execution
                .as_mut()
                .ok_or("Parked interface has no scene execution")?;
            let active = if execution
                .active
                .as_ref()
                .is_some_and(|active| active.id == activation && active.tasks.resources_parked)
            {
                execution.active.as_mut().unwrap()
            } else {
                execution
                    .frames
                    .iter_mut()
                    .find_map(|frame| {
                        frame.parked.as_mut().filter(|active| {
                            active.id == activation && active.tasks.resources_parked
                        })
                    })
                    .ok_or("Actual parked interface visit is absent")?
            };
            &mut active.tasks.suspended_ui
        }
    };
    ui.screens
        .iter_mut()
        .find(|instance| instance.name == target.alias && instance.order == order)
        .ok_or_else(|| "Actual interface snapshot is absent".into())
}
pub(crate) fn has_instance_consumers(state: &crate::GameState) -> bool {
    let has = |ui: &crate::ui::UiState| {
        ui.screens.iter().any(|instance| {
            !instance.consumer_frames.is_empty() || instance.consumer_projection.is_some()
        })
    };
    has(&state.ui)
        || state.scene_execution.as_ref().is_some_and(|execution| {
            execution
                .active
                .iter()
                .chain(
                    execution
                        .frames
                        .iter()
                        .filter_map(|frame| frame.parked.as_ref()),
                )
                .any(|active| has(&active.tasks.suspended_ui))
        })
}

/// In-process capture/restore permission. No deserializer or public constructor
/// can turn a logical saved definition name into this runtime capability.
pub(crate) struct HostContinuationPermit {
    allowed: BTreeSet<String>,
}
impl HostContinuationPermit {
    pub(crate) fn from_resident(
        program: &ConsumerProgram,
        state: &crate::GameState,
        target: &TargetIdentity,
    ) -> EvalResult<Self> {
        resident_interface(state, target).map_err(|s| invalid(&s))?;
        let source = program
            .source
            .as_ref()
            .ok_or_else(|| invalid("Host continuation has no frozen source"))?;
        let allowed = source.definitions.keys().cloned().collect();
        Ok(Self { allowed })
    }
    pub(crate) fn admits(&self, scope: &str) -> bool {
        self.allowed.contains(scope)
    }
}

/// Internal evaluator scope admission, never a serializable host capability.
/// Implementors are actual owner adapters; data labels do not grant access.
pub(crate) trait PortableScopePermit{fn admits(&self,scope:&str)->bool;}
impl PortableScopePermit for HostContinuationPermit{fn admits(&self,scope:&str)->bool{HostContinuationPermit::admits(self,scope)}}
