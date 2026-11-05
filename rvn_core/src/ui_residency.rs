//! Exact resident interface lookup independent of task clock ownership.
//! Exact resident UI identity and transient projection, never another clock.
use crate::scene::{NarrativeFrame, SceneActivation};
use crate::scene_resources::{
    ResourceRegistry, ResourceTask, ResourceTaskKind, TargetIdentity, TargetKind,
};
use crate::scene_tasks::{TaskKey, TaskOwner};
use crate::types::GameState;
use crate::ui::{ScreenInstance, UiState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionResidence {
    Visible,
    Parked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiProjectionBinding {
    pub interface: TargetIdentity,
    pub element: String,
    pub order: u64,
    pub host_role: Option<rvn_ui::PageRole>,
    pub residence: ProjectionResidence,
}

#[derive(Debug)]
pub struct ResidentInterface<'a> {
    pub identity: TargetIdentity,
    pub owner: TaskOwner,
    pub instance: &'a ScreenInstance,
    pub residence: ProjectionResidence,
}

fn eligible(owner: &TaskOwner, active: Option<&SceneActivation>) -> bool {
    match owner {
        TaskOwner::Scene { activation } => active.is_some_and(|scene| scene.id == *activation),
        TaskOwner::Game | TaskOwner::Host { .. } => true,
    }
}

/// Separated arguments permit ResourceRegistry's sync transaction to retain
/// short mutable registry borrows while consulting immutable scoped snapshots.
pub(crate) fn resident_interface_parts<'a>(
    ui: &'a UiState,
    registry: &ResourceRegistry,
    active: Option<&'a SceneActivation>,
    frames: &'a [NarrativeFrame],
    identity: &TargetIdentity,
) -> Result<ResidentInterface<'a>, String> {
    if identity.kind != TargetKind::Interface {
        return Err("The projection target is not an interface incarnation".into());
    }
    let record = registry
        .targets
        .iter()
        .find(|record| record.identity == *identity)
        .ok_or("The exact interface incarnation is no longer resident")?;
    let mut tasks = registry.tasks.iter().filter(|task| {
        task.kind == ResourceTaskKind::Interface
            && task.target == *identity
            && task.key.owner == record.owner
            && task.alias() == identity.alias
    });
    let task = tasks
        .next()
        .ok_or("The resident interface has no exact instance task")?;
    if tasks.next().is_some() {
        return Err("The interface incarnation has more than one instance task".into());
    }
    let (source, residence) = if task.parked {
        let TaskOwner::Scene { activation } = &record.owner else {
            return Err("A Game or Host interface cannot be parked with a scene".into());
        };
        let parent = active
            .filter(|scene| scene.id == *activation && scene.tasks.resources_parked)
            .or_else(|| {
                frames
                    .iter()
                    .filter_map(|frame| frame.parked.as_ref())
                    .find(|scene| scene.id == *activation && scene.tasks.resources_parked)
            })
            .ok_or("The interface task has lost its suspended visit")?;
        (&parent.tasks.suspended_ui, ProjectionResidence::Parked)
    } else {
        if !eligible(&record.owner, active)
            || active.is_some_and(|scene| {
                record.owner
                    == TaskOwner::Scene {
                        activation: scene.id,
                    }
                    && scene.tasks.resources_parked
            })
        {
            return Err("The interface task has no eligible visible presenter".into());
        }
        (ui, ProjectionResidence::Visible)
    };
    let mut screens = source
        .screens
        .iter()
        .filter(|screen| screen.name == identity.alias);
    let instance = screens
        .next()
        .ok_or("The resident interface has lost its exact snapshot")?;
    if screens.next().is_some() {
        return Err("The resident snapshot repeats an interface address".into());
    }
    let expected_role = match &record.owner {
        TaskOwner::Host { role } => {
            Some(rvn_ui::PageRole::from_id(role).ok_or("Invalid Host interface role")?)
        }
        _ => None,
    };
    if instance.host_role != expected_role {
        return Err("The resident interface snapshot belongs to another presenter".into());
    }
    Ok(ResidentInterface {
        identity: record.identity.clone(),
        owner: record.owner.clone(),
        instance,
        residence,
    })
}

pub fn resident_interface<'a>(
    state: &'a GameState,
    identity: &TargetIdentity,
) -> Result<ResidentInterface<'a>, String> {
    if state.scene_execution.is_none(){let registry=state.ui.instance_registry.as_ref().ok_or("Actual interface residency is unavailable")?;registry.validate_interface_registry(&state.ui)?;return resident_interface_parts(&state.ui,registry,None,&[],identity);}
    let execution = state.scene_execution.as_ref().unwrap();
    resident_interface_parts(
        &state.ui,
        &execution.resources,
        execution.active.as_ref(),
        &execution.frames,
        identity,
    )
}

/// A new command may bind only the currently visible instance. Existing tasks
/// instead keep their captured identity, even when that instance is parked.
pub fn visible_interface_target(state: &GameState, screen: &str) -> Result<TargetIdentity, String> {
    if state.scene_execution.is_none(){let registry=state.ui.instance_registry.as_ref().ok_or("Actual interface residency is unavailable")?;registry.validate_interface_registry(&state.ui)?;return visible_interface_target_parts(&state.ui,registry,None,&[],screen);}
    let execution = state.scene_execution.as_ref().unwrap();
    visible_interface_target_parts(
        &state.ui,
        &execution.resources,
        execution.active.as_ref(),
        &execution.frames,
        screen,
    )
}

pub(crate) fn visible_interface_target_parts(
    ui: &UiState,
    registry: &ResourceRegistry,
    active: Option<&SceneActivation>,
    frames: &[NarrativeFrame],
    screen: &str,
) -> Result<TargetIdentity, String> {
    let mut tasks = registry.tasks.iter().filter(|task| {
        task.kind == ResourceTaskKind::Interface
            && task.alias() == screen
            && !task.parked
            && eligible(&task.key.owner, active)
    });
    let task = tasks
        .next()
        .ok_or("A new command cannot address an absent or parked interface")?;
    if tasks.next().is_some() {
        return Err("The visible interface address is ambiguous".into());
    }
    let resident = resident_interface_parts(ui, registry, active, frames, &task.target)?;
    if resident.residence != ProjectionResidence::Visible {
        return Err("A new command cannot address a parked interface".into());
    }
    Ok(resident.identity)
}

pub(crate) fn video_binding_parts(
    ui: &UiState,
    registry: &ResourceRegistry,
    active: Option<&SceneActivation>,
    frames: &[NarrativeFrame],
    task: &ResourceTask,
    track: &crate::video::Track,
) -> Result<Option<UiProjectionBinding>, String> {
    let Some(target) = &track.clip.target else {
        if !task.dependencies.is_empty() {
            return Err("A standalone video retains an obsolete interface dependency".into());
        }
        return Ok(None);
    };
    let (screen, element) = target
        .strip_prefix("ui:")
        .and_then(|text| text.split_once('/'))
        .ok_or("Invalid embedded video target")?;
    if !rvn_ui::composition::valid_name(element) {
        return Err("Invalid embedded video component identity".into());
    }
    let [interface] = task.dependencies.as_slice() else {
        return Err(
            "An embedded player must retain exactly one captured interface dependency".into(),
        );
    };
    if interface.kind != TargetKind::Interface || interface.alias != screen {
        return Err("The player descriptor addresses a different captured interface".into());
    }
    binding_parts(ui, registry, active, frames, interface, element)
}

pub(crate) fn motion_binding_parts(
    ui: &UiState,
    registry: &ResourceRegistry,
    active: Option<&SceneActivation>,
    frames: &[NarrativeFrame],
    task: &ResourceTask,
    track: &crate::motion::Track,
) -> Result<Option<UiProjectionBinding>, String> {
    let crate::motion::MotionTarget::Interface { screen, element } = &track.target else {
        return Ok(None);
    };
    if task.target.kind != TargetKind::Interface || task.target.alias != *screen {
        return Err("The motion descriptor addresses a different captured interface".into());
    }
    binding_parts(ui, registry, active, frames, &task.target, element)
}

fn binding_parts(
    ui: &UiState,
    registry: &ResourceRegistry,
    active: Option<&SceneActivation>,
    frames: &[NarrativeFrame],
    interface: &TargetIdentity,
    element: &str,
) -> Result<Option<UiProjectionBinding>, String> {
    if !rvn_ui::composition::valid_name(element) {
        return Err("Invalid projection component identity".into());
    }
    let resident = resident_interface_parts(ui, registry, active, frames, interface)?;
    Ok(Some(UiProjectionBinding {
        interface: resident.identity,
        element: element.into(),
        order: resident.instance.order,
        host_role: resident.instance.host_role,
        residence: resident.residence,
    }))
}

/// Describe only an exact live operation. Parked Scene operations are not
/// renderer inputs; Game operations may target a resident parked Scene UI.
pub fn interface_projection(
    state: &GameState,
    key: &TaskKey,
) -> Result<Option<UiProjectionBinding>, String> {
    let execution = state
        .scene_execution
        .as_ref()
        .ok_or("Narrative residency is unavailable")?;
    let task = execution
        .resources
        .tasks
        .iter()
        .find(|task| task.key == *key)
        .ok_or("The exact operation is no longer resident")?;
    if task.parked || !eligible(&task.key.owner, execution.active.as_ref()) {
        return Err("A parked operation cannot become a live projection".into());
    }
    match task.kind {
        ResourceTaskKind::Video => video_binding_parts(
            &state.ui,
            &execution.resources,
            execution.active.as_ref(),
            &execution.frames,
            task,
            state
                .videos
                .tracks
                .get(task.alias())
                .ok_or("The live player descriptor is absent")?,
        ),
        ResourceTaskKind::Motion => motion_binding_parts(
            &state.ui,
            &execution.resources,
            execution.active.as_ref(),
            &execution.frames,
            task,
            state
                .motions
                .tracks
                .get(task.alias())
                .ok_or("The live motion descriptor is absent")?,
        ),
        _ => Err("Only video and motion operations have an interface projection binding".into()),
    }
}

/// Explicit producer binding on an Engine candidate before its final sync.
/// This intentionally does not project/evaluate widgets or advance a clock.
/// Final state/scene batch validation remains the single commit boundary.
pub fn bind_video_interface(state: &mut GameState, key: &TaskKey) -> Result<(), String> {
    let execution = state
        .scene_execution
        .as_ref()
        .ok_or("Narrative residency is unavailable")?;
    let task = execution
        .resources
        .tasks
        .iter()
        .find(|task| task.key == *key)
        .ok_or("The new player has no exact task")?;
    if task.kind != ResourceTaskKind::Video
        || task.parked
        || !eligible(&task.key.owner, execution.active.as_ref())
    {
        return Err("Only an eligible player can bind a visible interface".into());
    }
    let track = state
        .videos
        .tracks
        .get(task.alias())
        .ok_or("The player descriptor is absent")?;
    let target = track
        .clip
        .target
        .as_ref()
        .ok_or("The new player is not embedded")?;
    let (screen, element) = target
        .strip_prefix("ui:")
        .and_then(|text| text.split_once('/'))
        .ok_or("Invalid embedded video target")?;
    if !rvn_ui::composition::valid_name(element) {
        return Err("Invalid embedded video component identity".into());
    }
    let identity = visible_interface_target(state, screen)?;
    if !task.dependencies.is_empty() && task.dependencies != [identity.clone()] {
        return Err("A player must be superseded before its interface binding changes".into());
    }
    let index = execution
        .resources
        .tasks
        .iter()
        .position(|task| task.key == *key)
        .unwrap();
    state.scene_execution.as_mut().unwrap().resources.tasks[index].dependencies = vec![identity];
    Ok(())
}

/// Saved descriptors may refer to any exact resident Story/Host interface.
/// Live Game tasks are allowed to refer to parked Scene snapshots; this is
/// distinct from permitting a new command to discover that parked snapshot.
pub(crate) fn validate_resident_projections(state: &GameState) -> Result<(), String> {
    let Some(execution) = &state.scene_execution else {
        return Ok(());
    };
    let check = |scope: Option<TaskOwner>,
                 parked: bool,
                 motions: &crate::motion::MotionState,
                 videos: &crate::video::VideoState|
     -> Result<(), String> {
        let task = |kind, alias: &str| {
            execution
                .resources
                .tasks
                .iter()
                .find(|task| {
                    task.kind == kind
                        && task.alias() == alias
                        && task.parked == parked
                        && scope.as_ref().map_or_else(
                            || eligible(&task.key.owner, execution.active.as_ref()),
                            |scope| task.key.owner == *scope,
                        )
                })
                .ok_or_else(|| "The projected descriptor has lost its exact operation".to_string())
        };
        for (alias, track) in &motions.tracks {
            motion_binding_parts(
                &state.ui,
                &execution.resources,
                execution.active.as_ref(),
                &execution.frames,
                task(ResourceTaskKind::Motion, alias)?,
                track,
            )?;
        }
        for (alias, track) in &videos.tracks {
            video_binding_parts(
                &state.ui,
                &execution.resources,
                execution.active.as_ref(),
                &execution.frames,
                task(ResourceTaskKind::Video, alias)?,
                track,
            )?;
        }
        Ok(())
    };
    check(None, false, &state.motions, &state.videos)?;
    if let Some(active) = &execution.active {
        check(
            Some(TaskOwner::Scene {
                activation: active.id,
            }),
            true,
            &active.tasks.suspended_motions,
            &active.tasks.suspended_videos,
        )?;
    }
    for frame in &execution.frames {
        if let Some(parent) = &frame.parked {
            check(
                Some(TaskOwner::Scene {
                    activation: parent.id,
                }),
                true,
                &parent.tasks.suspended_motions,
                &parent.tasks.suspended_videos,
            )?;
        }
    }
    Ok(())
}
