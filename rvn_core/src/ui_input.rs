//! Transient input authority captured by the producer, never by the dequeue path.
//! No source or queued input is a member of GameState or SaveData.
use crate::scene_resources::{ResourceTaskKind, ResourceToken, TargetIdentity, TargetKind};
use crate::scene_tasks::{TaskKey, TaskToken};
use crate::types::GameState;
use crate::ui::UiInput;
use crate::ui_residency::{resident_interface, visible_interface_target, ProjectionResidence};
use serde::{Deserialize, Serialize};

// Process-local lineage survives queued input after an Engine is dropped.
// Pointer/renderer addresses can be reused; saved runtime counters can restart.
static NEXT_LINEAGE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
fn allocate_lineage(counter: &std::sync::atomic::AtomicUsize) -> Result<u64, crate::EvalError> {
    use std::sync::atomic::Ordering;
    counter.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|next|next.checked_add(1))
        .map(|reserved|reserved as u64).map_err(|_|crate::EvalError::NumericOverflow)
}
pub(crate) fn new_engine_lineage() -> Result<u64,crate::EvalError> { allocate_lineage(&NEXT_LINEAGE) }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiInputAuthority {
    Narrative(ResourceToken),
    Instance {token:ResourceToken,interface_epoch:u64},
    Legacy { interface_epoch: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiInputSource {
    pub lineage: u64,
    pub screen: String,
    pub order: u64,
    pub host_role: Option<rvn_ui::PageRole>,
    pub authority: UiInputAuthority,
}

#[derive(Clone, Debug)]
pub struct ScopedUiInput {
    pub source: UiInputSource,
    pub event: UiInput,
}

// This DTO is an external transient envelope, not serde on ResourceToken.
// Browser code carries its JSON as an opaque string: no u64 passes through JS Number.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceWire {
    lineage: u64,
    screen: String,
    order: u64,
    host_role: Option<rvn_ui::PageRole>,
    authority: AuthorityWire,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum AuthorityWire {
    Narrative { runtime_epoch: u64, key: TaskKey, revision: u64, target: TargetIdentity },
    Instance { runtime_epoch:u64,key:TaskKey,revision:u64,target:TargetIdentity,interface_epoch:u64 },
    Legacy { interface_epoch: u64 },
}
impl UiInputSource {
    pub fn to_wire(&self) -> Result<String, String> {
        let authority = match &self.authority {
            UiInputAuthority::Narrative(token) => AuthorityWire::Narrative {
                runtime_epoch: token.task.runtime_epoch, key: token.task.key.clone(),
                revision: token.task.revision, target: token.target.clone(),
            },
            UiInputAuthority::Legacy { interface_epoch } => AuthorityWire::Legacy { interface_epoch: *interface_epoch },
            UiInputAuthority::Instance {token,interface_epoch} => AuthorityWire::Instance {runtime_epoch:token.task.runtime_epoch,key:token.task.key.clone(),revision:token.task.revision,target:token.target.clone(),interface_epoch:*interface_epoch},
        };
        serde_json::to_string(&SourceWire { lineage: self.lineage, screen: self.screen.clone(), order: self.order, host_role: self.host_role, authority })
            .map_err(|error| format!("Interface input source: {error}"))
    }
    pub fn from_wire(text: &str) -> Result<Self, String> {
        if text.len() > 4096 { return Err("Interface input source exceeds 4096 bytes".into()); }
        let wire: SourceWire = serde_json::from_str(text).map_err(|error| format!("Interface input source: {error}"))?;
        if wire.screen.len() > 128 { return Err("Invalid interface input source address".into()); }
        let authority = match wire.authority {
            AuthorityWire::Narrative { runtime_epoch, key, revision, target } => {
                if key.local_id.len() > 512 || target.alias.len() > 128 {
                    return Err("Invalid interface input source identity".into());
                }
                UiInputAuthority::Narrative(ResourceToken { task: TaskToken { runtime_epoch, key, revision }, target })
            },
            AuthorityWire::Legacy { interface_epoch } => UiInputAuthority::Legacy { interface_epoch },
            AuthorityWire::Instance {runtime_epoch,key,revision,target,interface_epoch} => {
                if key.local_id.len()>512||target.alias.len()>128{return Err("Invalid interface input instance identity".into());}
                UiInputAuthority::Instance {token:ResourceToken{task:TaskToken{runtime_epoch,key,revision},target},interface_epoch}
            },
        };
        Ok(Self { lineage: wire.lineage, screen: wire.screen, order: wire.order, host_role: wire.host_role, authority })
    }
}

pub(crate) fn capture(state: &GameState, lineage: u64, runtime_epoch: u64, interface_epoch: u64, screen: &str) -> Option<UiInputSource> {
    let instance=state.ui.screens.iter().find(|instance|instance.name==screen)?;
    if !instance.viewport_attached()||instance.widget_class.as_ref().is_some_and(|binding|binding.pending_materialization){return None;}
    capture_logical(state,lineage,runtime_epoch,interface_epoch,screen)
}
/// Internal saved continuation admission, not a presentation/input producer.
/// The caller must compare frozen method/target data before retaining it.
pub(crate) fn capture_logical(state:&GameState,lineage:u64,runtime_epoch:u64,interface_epoch:u64,screen:&str)->Option<UiInputSource>{
    if !state.ui.screens.iter().find(|instance|instance.name==screen)?.viewport_attached(){return None;}
    crate::scene_resources::validate_ui_state(&state.ui,&mut crate::scene::NarrativeBudget::default()).ok()?;
    if state.scene_execution.is_some() {
        let target = visible_interface_target(state, screen).ok()?;
        let resident = resident_interface(state, &target).ok()?;
        if resident.residence != ProjectionResidence::Visible { return None; }
        let execution = state.scene_execution.as_ref()?;
        let task = execution.resources.task(ResourceTaskKind::Interface, screen, execution.owner())?;
        if task.target != target { return None; }
        let token = execution.resources.effect_token(runtime_epoch, &task.key, execution.owner())?;
        Some(UiInputSource { lineage, screen: screen.into(), order: resident.instance.order, host_role: resident.instance.host_role, authority: UiInputAuthority::Narrative(token) })
    } else if let Some(registry)=&state.ui.instance_registry{
        registry.validate_interface_registry(&state.ui).ok()?;
        let target=visible_interface_target(state,screen).ok()?;let resident=resident_interface(state,&target).ok()?;
        let task=registry.task(ResourceTaskKind::Interface,screen,None)?;if task.target!=target{return None;}
        let token=registry.effect_token(runtime_epoch,&task.key,None)?;
        Some(UiInputSource{lineage,screen:screen.into(),order:resident.instance.order,host_role:resident.instance.host_role,authority:UiInputAuthority::Instance{token,interface_epoch}})
    } else {
        let mut screens = state.ui.screens.iter().filter(|instance| instance.name == screen);
        let instance = screens.next()?;
        if screens.next().is_some() { return None; }
        Some(UiInputSource { lineage, screen: screen.into(), order: instance.order, host_role: instance.host_role, authority: UiInputAuthority::Legacy { interface_epoch } })
    }
}

/// Re-admit the producing presentation at dequeue time as well as capture.
/// A resident logical object may have lost its viewport or await reconstruction.
pub(crate) fn current(state: &GameState, lineage: u64, runtime_epoch: u64, interface_epoch: u64, source: &UiInputSource) -> bool {
    current_logical(state,lineage,runtime_epoch,interface_epoch,source)
        && state.ui.screens.iter().find(|instance|instance.name==source.screen)
            .is_some_and(|instance|instance.widget_class.as_ref().is_none_or(|binding|!binding.pending_materialization))
}
/// Only the existing saved, frozen Host continuation uses this logical check.
/// Pending reconstruction is not an external input capability. Attachment,
/// exact owner/incarnation/revision/order/lineage remain mandatory here.
pub(crate) fn current_logical(state: &GameState, lineage: u64, runtime_epoch: u64, interface_epoch: u64, source: &UiInputSource) -> bool {
    if source.lineage != lineage { return false; }
    if crate::scene_resources::validate_ui_state(&state.ui,&mut crate::scene::NarrativeBudget::default()).is_err() { return false; }
    match &source.authority {
        UiInputAuthority::Narrative(token) => {
            let Some(execution) = state.scene_execution.as_ref() else { return false; };
            if token.target.kind != TargetKind::Interface || token.target.alias != source.screen
                || !execution.resources.accepts(token, runtime_epoch, execution.owner()) { return false; }
            let Some(task) = execution.resources.tasks.iter().find(|task| task.key == token.task.key) else { return false; };
            if task.kind != ResourceTaskKind::Interface { return false; }
            let Ok(resident) = resident_interface(state, &token.target) else { return false; };
            resident.residence == ProjectionResidence::Visible && resident.instance.viewport_attached() && resident.instance.order == source.order
                && resident.instance.host_role == source.host_role && resident.instance.name == source.screen
        },
        UiInputAuthority::Legacy { interface_epoch: captured } => {
            if state.scene_execution.is_some() || state.ui.instance_registry.is_some() || *captured != interface_epoch { return false; }
            let mut screens = state.ui.screens.iter().filter(|instance| instance.name == source.screen);
            screens.next().is_some_and(|instance| instance.viewport_attached() && instance.order == source.order && instance.host_role == source.host_role)
                && screens.next().is_none()
        },
        UiInputAuthority::Instance {token,interface_epoch:captured} => {
            if state.scene_execution.is_some()||*captured!=interface_epoch||token.target.kind!=TargetKind::Interface||token.target.alias!=source.screen{return false;}
            let Some(registry)=&state.ui.instance_registry else{return false};if registry.validate_interface_registry(&state.ui).is_err()||!registry.accepts(token,runtime_epoch,None){return false;}
            let Ok(resident)=resident_interface(state,&token.target)else{return false};resident.residence==ProjectionResidence::Visible&&resident.instance.viewport_attached()&&resident.instance.order==source.order&&resident.instance.host_role==source.host_role&&resident.instance.name==source.screen
        },
    }
}

#[cfg(test)]
mod lineage_contracts {
    use super::*;
    #[test]
    fn scoped_input_lineage_overflow_refuses_without_reuse_or_counter_mutation() {
        use std::sync::atomic::{AtomicUsize,Ordering};
        let counter=AtomicUsize::new(usize::MAX-1);
        assert_eq!(allocate_lineage(&counter).unwrap(),(usize::MAX-1) as u64);
        assert_eq!(counter.load(Ordering::Relaxed),usize::MAX);
        assert!(matches!(allocate_lineage(&counter),Err(crate::EvalError::NumericOverflow)));
        assert_eq!(counter.load(Ordering::Relaxed),usize::MAX);
    }
}
