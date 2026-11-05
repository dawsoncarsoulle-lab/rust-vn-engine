//! Narrative visits, distinct from backgrounds, editor tabs and lexical functions.
//!
//! This data is authoritative and serializable. Renderer handles and callback
//! generations deliberately do not belong here.
use crate::eval::EvalError;
#[path="scene_native_loops.rs"]
pub mod native_loops;
use native_loops::NativeLoopFramesV1;
use crate::program::{InstructionOrigin, PreparedNarrative};
use rvn_parser::Value;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const MAX_NARRATIVE_FRAMES: usize = 128;
pub const MAX_NARRATIVE_BINDINGS: usize = 4096;
pub const MAX_NARRATIVE_VALUE_WORK: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActivationId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumePosition {
    pub pc: usize,
    pub origin: Option<InstructionOrigin>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneActivation {
    pub id: ActivationId,
    pub scene: String,
    pub entry_pc: usize,
    pub locals: HashMap<String, Value>,
    /// Compiler-created iteration state never enters the game globals.
    pub temporaries: HashMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumer_frame: Option<crate::consumer_schedule::BoundConsumerFrame>,
    pub tasks: crate::scene_tasks::SceneTasks,
    #[serde(default,skip_serializing_if="NativeLoopFramesV1::is_empty")]
    pub native_loop_frames_v1:NativeLoopFramesV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameKind {
    Branch,
    Helper,
    SceneCall,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrativeFrame {
    pub kind: FrameKind,
    pub continuation: ResumePosition,
    pub owner: Option<ActivationId>,
    pub called_activation: Option<ActivationId>,
    pub parked: Option<SceneActivation>,
    pub pre_scene_temporaries: Option<HashMap<String, Value>>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub parked_native_loops_v1:Option<NativeLoopFramesV1>,
    #[serde(default)]
    pub pre_scene_resources: Option<crate::scene_tasks::SceneTasks>,
}

/// Captured display input is independent from a callback's live continuation.
/// Redrawing it never replays initializers or advances either random stream.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayCheckpoint {
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub consumer_dialogue:Option<crate::consumer_schedule::BoundConsumerDialogue>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub consumer_choice:Option<crate::consumer_schedule::BoundConsumerChoice>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub consumer_imagemap:Option<crate::consumer_schedule::BoundConsumerImagemap>,
    pub position: ResumePosition,
    pub variables: HashMap<String, Value>,
    pub lexical_globals: HashMap<String, Value>,
    pub random: crate::random::RandomState,
    pub typewriter: crate::types::TypewriterState,
    pub display: crate::rollback::HistoryDisplay,
    pub choice_indices: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reveal: Option<crate::display_reveal::RevealCheckpoint>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneExecution {
    pub next_activation_id: u64,
    pub active: Option<SceneActivation>,
    pub frames: Vec<NarrativeFrame>,
    pub pre_scene_temporaries: HashMap<String, Value>,
    #[serde(default,skip_serializing_if="NativeLoopFramesV1::is_empty")]
    pub native_loop_frames_v1:NativeLoopFramesV1,
    pub live_position: ResumePosition,
    pub display: Option<DisplayCheckpoint>,
    pub game_tasks: crate::scene_tasks::SceneTasks,
    #[serde(default)]
    pub resources: crate::scene_resources::ResourceRegistry,
    #[serde(default)]
    pub sprite_styles: HashMap<String, crate::sprite_effects::SpriteStyle>,
}

impl SceneExecution {
    pub fn new(program: &PreparedNarrative) -> Self {
        Self {
            next_activation_id: 1,
            active: None,
            frames: Vec::new(),
            pre_scene_temporaries: HashMap::new(),
            native_loop_frames_v1:Default::default(),
            live_position: program.position(0),
            display: None,
            game_tasks: Default::default(),
            resources: Default::default(),
            sprite_styles: Default::default(),
        }
    }

    pub fn owner(&self) -> Option<ActivationId> {
        self.active.as_ref().map(|activation| activation.id)
    }

    fn activate(&mut self, scene: &str, pc: usize) -> Result<ActivationId, EvalError> {
        let id = ActivationId(self.next_activation_id);
        self.next_activation_id = self
            .next_activation_id
            .checked_add(1)
            .ok_or(EvalError::NumericOverflow)?;
        self.active = Some(SceneActivation {
            id,
            scene: scene.into(),
            entry_pc: pc,
            locals: HashMap::new(),
            temporaries: HashMap::new(),
            consumer_frame: None,
            tasks: Default::default(),
            native_loop_frames_v1:Default::default(),
        });
        Ok(id)
    }

    /// A jump abandons the visit's helper/branch frames, retaining a real caller.
    pub fn enter(&mut self, scene: &str, pc: usize) -> Result<(), EvalError> {
        let abandoned_owner = self.owner();
        self.frames
            .retain(|frame| frame.kind == FrameKind::SceneCall || frame.owner != abandoned_owner);
        self.activate(scene, pc)?;
        if abandoned_owner.is_none() {
            self.pre_scene_temporaries.clear();
            self.native_loop_frames_v1.0.clear();
        }
        // A scene call can have changed its callee through a jump. The frame
        // records the live callee, not the no-longer-existing previous visit.
        let owner = self.owner();
        if let Some(frame) = self
            .frames
            .iter_mut()
            .rev()
            .find(|frame| frame.kind == FrameKind::SceneCall)
        {
            frame.called_activation = owner;
        }
        Ok(())
    }

    pub fn call_scene(
        &mut self,
        scene: &str,
        pc: usize,
        continuation: ResumePosition,
    ) -> Result<(), EvalError> {
        self.check_depth()?;
        // Allocate before moving the parent so overflow leaves the candidate
        // coherent; callers still publish the whole candidate only on success.
        let mut parent = self.active.clone();
        if let Some(parent) = &mut parent {
            parent.tasks.renew_callbacks()?;
        }
        let owner = self.owner();
        let child = self.activate(scene, pc)?;
        self.frames.push(NarrativeFrame {
            kind: FrameKind::SceneCall,
            continuation,
            owner,
            called_activation: Some(child),
            parked: parent,
            parked_native_loops_v1:if owner.is_none()&&!self.native_loop_frames_v1.is_empty(){Some(std::mem::take(&mut self.native_loop_frames_v1))}else{None},
            pre_scene_resources: if owner.is_none() && self.game_tasks.resources_parked {
                self.game_tasks.resources_parked = false;
                Some(crate::scene_tasks::SceneTasks {
                    suspended_motions: std::mem::take(&mut self.game_tasks.suspended_motions),
                    suspended_videos: std::mem::take(&mut self.game_tasks.suspended_videos),
                    suspended_ui: std::mem::take(&mut self.game_tasks.suspended_ui),
                    resources_parked: true,
                    ..Default::default()
                })
            } else {
                None
            },
            pre_scene_temporaries: if owner.is_none() {
                Some(std::mem::take(&mut self.pre_scene_temporaries))
            } else {
                None
            },
        });
        Ok(())
    }

    pub fn call_helper(
        &mut self,
        continuation: ResumePosition,
        synthetic: bool,
    ) -> Result<(), EvalError> {
        self.check_depth()?;
        let parked_native_loops_v1=if !synthetic&&!self.native_loop_frames().is_empty(){Some(std::mem::take(self.native_loop_frames_mut()))}else{None};
        self.frames.push(NarrativeFrame {
            parked_native_loops_v1,
            kind: if synthetic {
                FrameKind::Branch
            } else {
                FrameKind::Helper
            },
            continuation,
            owner: self.owner(),
            called_activation: None,
            parked: None,
            pre_scene_temporaries: None,
            pre_scene_resources: None,
        });
        Ok(())
    }

    fn check_depth(&self) -> Result<(), EvalError> {
        if self.frames.len() >= MAX_NARRATIVE_FRAMES {
            Err(EvalError::ExecutionLimit {
                limit: "128 appels narratifs imbriqués",
            })
        } else {
            Ok(())
        }
    }

    /// Synthetic branch completion pops one frame. An authored return leaves
    /// branch scaffolding and then returns from the nearest real call.
    pub fn return_to_caller(
        &mut self,
        synthetic: bool,
    ) -> Result<Option<ResumePosition>, EvalError> {
        if !synthetic {
            while self
                .frames
                .last()
                .is_some_and(|frame| frame.kind == FrameKind::Branch)
            {
                self.frames.pop();
            }
        }
        let Some(frame) = self.frames.pop() else {
            return Ok(None);
        };
        if frame.kind == FrameKind::SceneCall {
            self.active = frame.parked;
            if let Some(parent) = &mut self.active {
                parent.tasks.renew_callbacks()?;
            }
            if let Some(resources) = frame.pre_scene_resources {
                self.game_tasks.suspended_motions = resources.suspended_motions;
                self.game_tasks.suspended_videos = resources.suspended_videos;
                self.game_tasks.suspended_ui = resources.suspended_ui;
                self.game_tasks.resources_parked = resources.resources_parked;
            }
            if let Some(temporaries) = frame.pre_scene_temporaries {
                self.pre_scene_temporaries = temporaries;
            }
        }
        if frame.kind==FrameKind::Helper {
            *self.native_loop_frames_mut()=frame.parked_native_loops_v1.unwrap_or_default();
        }else if frame.kind==FrameKind::SceneCall&&frame.owner.is_none(){
            self.native_loop_frames_v1=frame.parked_native_loops_v1.unwrap_or_default();
        }
        Ok(Some(frame.continuation))
    }

    pub fn peek_return_kind(&self, synthetic: bool) -> Option<FrameKind> {
        self.frames
            .iter()
            .rev()
            .find(|frame| synthetic || frame.kind != FrameKind::Branch)
            .map(|frame| frame.kind)
    }

    pub fn derived_call_stack(&self) -> Vec<usize> {
        self.frames
            .iter()
            .map(|frame| frame.continuation.pc)
            .collect()
    }

    pub(crate) fn visit_native_loop_frames<E>(&self,mut visit:impl FnMut(&native_loops::NativeLoopFrame)->Result<(),E>)->Result<(),E>{
        for frame in &self.native_loop_frames_v1.0{visit(frame)?;}
        if let Some(active)=&self.active{for frame in &active.native_loop_frames_v1.0{visit(frame)?;}}
        for continuation in &self.frames {
            if let Some(activation)=&continuation.parked{for frame in &activation.native_loop_frames_v1.0{visit(frame)?;}}
            if let Some(loops)=&continuation.parked_native_loops_v1{for frame in &loops.0{visit(frame)?;}}
        }
        Ok(())
    }
    pub(crate) fn has_native_loop_frames(&self)->bool{
        !self.native_loop_frames_v1.is_empty()||self.active.as_ref().is_some_and(|a|!a.native_loop_frames_v1.is_empty())||self.frames.iter().any(|f|f.parked.as_ref().is_some_and(|a|!a.native_loop_frames_v1.is_empty())||f.parked_native_loops_v1.as_ref().is_some_and(|loops|!loops.is_empty()))
    }
    pub(crate) fn visit_native_loop_frames_mut<E>(&mut self,mut visit:impl FnMut(&mut native_loops::NativeLoopFrame)->Result<(),E>)->Result<(),E>{
        for frame in &mut self.native_loop_frames_v1.0{visit(frame)?;}
        if let Some(active)=&mut self.active{for frame in &mut active.native_loop_frames_v1.0{visit(frame)?;}}
        for continuation in &mut self.frames {
            if let Some(activation)=&mut continuation.parked{for frame in &mut activation.native_loop_frames_v1.0{visit(frame)?;}}
            if let Some(loops)=&mut continuation.parked_native_loops_v1{for frame in &mut loops.0{visit(frame)?;}}
        }
        Ok(())
    }
    pub(crate) fn native_loop_frames(&self)->&NativeLoopFramesV1 {
        match &self.active{Some(activation)=>&activation.native_loop_frames_v1,None=>&self.native_loop_frames_v1}
    }
    pub(crate) fn native_loop_frames_mut(&mut self)->&mut NativeLoopFramesV1 {
        match &mut self.active{Some(activation)=>&mut activation.native_loop_frames_v1,None=>&mut self.native_loop_frames_v1}
    }
    pub fn temporaries_mut(&mut self) -> &mut HashMap<String, Value> {
        match self.active.as_mut() {
            Some(activation) => &mut activation.temporaries,
            None => &mut self.pre_scene_temporaries,
        }
    }

    pub(crate) fn validate_sprite_styles(
        &self,
        budget: &mut NarrativeBudget,
    ) -> Result<(), String> {
        for (name, style) in &self.sprite_styles {
            if !rvn_ui::composition::valid_name(name) {
                return Err("Invalid Game sprite style identity".into());
            }
            style.validate()?;
            budget.presentation(
                1,
                serde_json::to_vec(style)
                    .map_err(|error| error.to_string())?
                    .len(),
            )?;
        }
        Ok(())
    }

    pub fn validate(
        &self,
        program: &PreparedNarrative,
        globals: &HashMap<String, Value>,
        call_stack: &[usize],
    ) -> Result<(), String> {
        if self.next_activation_id == 0 {
            return Err("Invalid next scene activation identity".into());
        }
        if self.frames.len() > MAX_NARRATIVE_FRAMES || self.derived_call_stack() != call_stack {
            return Err("Narrative continuation stack is incoherent".into());
        }
        program.validate_position(&self.live_position)?;
        let mut ids = HashSet::new();
        let mut budget = NarrativeBudget::default();
        budget.values(globals)?;
        self.validate_sprite_styles(&mut budget)?;
        budget.values(&self.pre_scene_temporaries)?;
        fn validate_activation(
            activation: &SceneActivation,
            program: &PreparedNarrative,
            next_id: u64,
            ids: &mut HashSet<ActivationId>,
            budget: &mut NarrativeBudget,
        ) -> Result<(), String> {
            if activation.id.0 == 0 || activation.id.0 >= next_id || !ids.insert(activation.id) {
                return Err("Scene activation identity is invalid or duplicated".into());
            }
            if program.scene_entries.get(&activation.entry_pc) != Some(&activation.scene) {
                return Err("Saved scene entry does not belong to this story".into());
            }
            if activation
                .locals
                .keys()
                .any(|name| !rvn_parser::is_variable_binding_name(name))
            {
                return Err("Invalid local scene binding".into());
            }
            budget.values(&activation.locals)?;
            budget.values(&activation.temporaries)?;
            if let Some(frame) = &activation.consumer_frame {
                if frame.owner != (crate::consumer_schedule::ConsumerOwner::Scene { activation: activation.id }) {
                    return Err("Scheduled consumer does not belong to its actual SceneActivation".into());
                }
                let storage = frame.frame.storage().map_err(|error| error.to_string())?;
                budget.presentation(0, storage.bytes)?;
            }
            activation.tasks.validate()?;
            Ok(())
        }
        if let Some(active) = &self.active {
            validate_activation(
                active,
                program,
                self.next_activation_id,
                &mut ids,
                &mut budget,
            )?;
        }
        for frame in &self.frames {
            program.validate_position(&frame.continuation)?;
            if let Some(parked) = &frame.parked {
                validate_activation(
                    parked,
                    program,
                    self.next_activation_id,
                    &mut ids,
                    &mut budget,
                )?;
            }
            if let Some(values) = &frame.pre_scene_temporaries {
                budget.values(values)?;
            }
            if frame.kind != FrameKind::SceneCall
                && (frame.parked.is_some()
                    || frame.called_activation.is_some()
                    || frame.pre_scene_temporaries.is_some()
                    || frame.pre_scene_resources.is_some())
            {
                return Err("A helper frame cannot contain a parked scene".into());
            }
            if frame.pre_scene_resources.is_some() && frame.owner.is_some() {
                return Err("Only a previsit caller may carry previsit resources".into());
            }
            if let Some(resources) = &frame.pre_scene_resources {
                resources.validate()?;
            }
            if frame.kind == FrameKind::SceneCall
                && (frame.called_activation.is_none()
                    || frame.parked.as_ref().map(|parent| parent.id) != frame.owner
                    || frame.pre_scene_temporaries.is_some() != frame.owner.is_none())
            {
                return Err("A scene call has lost its caller".into());
            }
        }
        // Validate the actual stack chain, rather than just accepting known IDs.
        let mut owner = self
            .frames
            .iter()
            .find(|frame| frame.kind == FrameKind::SceneCall)
            .map(|frame| frame.owner)
            .unwrap_or_else(|| self.owner());
        for frame in &self.frames {
            if frame.owner != owner {
                return Err("Narrative frame owner does not match its caller".into());
            }
            if frame.kind == FrameKind::SceneCall {
                owner = frame.called_activation;
            }
        }
        if owner != self.owner() {
            return Err("Active scene does not match the continuation chain".into());
        }
        if self.active.is_some()&&!self.native_loop_frames_v1.is_empty(){return Err("Pre-scene Native loops were not parked by the real scene call".into());}
        // One existing continuation budget covers every active and parked
        // gather, rather than allowing each nested frame its own full budget.
        let(mut retained_bytes,mut retained_work)=(0usize,0usize);
        self.visit_native_loop_frames(|frame|{
            for record in &frame.gather{record.account_native_loop_storage(&mut retained_bytes,&mut retained_work).map_err(|error|error.to_string())?;}
            Ok::<(),String>(())
        })?;
        self.native_loop_frames().validate_chain(program,&self.frames,self.owner(),&self.live_position).map_err(|error|error.to_string())?;
        for (index,frame) in self.frames.iter().enumerate(){
            if let Some(parked)=&frame.parked{
                parked.native_loop_frames_v1.validate_chain(program,&self.frames[..index],frame.owner,&frame.continuation).map_err(|error|error.to_string())?;
            }
            if let Some(loops)=&frame.parked_native_loops_v1 {
                if loops.is_empty()||frame.kind==FrameKind::Branch||frame.kind==FrameKind::SceneCall&&frame.owner.is_some(){return Err("Native loops parked on a foreign or empty continuation".into());}
                loops.validate_chain(program,&self.frames[..index],frame.owner,&frame.continuation).map_err(|error|error.to_string())?;
            }
        }
        self.game_tasks.validate()?;
        self.resources.validate(self)?;
        let mut resources = crate::scene_resources::ResourceBudget::default();
        resources.retired(&self.resources, &mut budget)?;
        if let Some(active) = &self.active {
            resources.snapshot(
                &active.tasks.suspended_motions,
                &active.tasks.suspended_videos,
                &active.tasks.suspended_ui,
                &mut budget,
            )?;
        }
        for frame in &self.frames {
            if let Some(previsit) = &frame.pre_scene_resources {
                resources.snapshot(
                    &previsit.suspended_motions,
                    &previsit.suspended_videos,
                    &previsit.suspended_ui,
                    &mut budget,
                )?;
            }
            if let Some(parent) = &frame.parked {
                resources.snapshot(
                    &parent.tasks.suspended_motions,
                    &parent.tasks.suspended_videos,
                    &parent.tasks.suspended_ui,
                    &mut budget,
                )?;
            }
        }
        resources.snapshot(
            &self.game_tasks.suspended_motions,
            &self.game_tasks.suspended_videos,
            &self.game_tasks.suspended_ui,
            &mut budget,
        )?;
        crate::scene_resources::validate_suspended_ownership(self)?;
        for wait in &self.resources.waits {
            program.validate_position(&wait.position)?;
        }
        for task in &self.resources.tasks {
            if let Some(position) = &task.wait {
                program.validate_position(position)?;
            }
        }
        for wait in &self.resources.cancelled_waits {
            program.validate_position(&wait.position)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct NarrativeBudget {
    bindings: usize,
    work: usize,
}
impl NarrativeBudget {
    pub(crate) fn presentation(&mut self, bindings: usize, work: usize) -> Result<(), String> {
        self.bindings = self
            .bindings
            .checked_add(bindings)
            .ok_or("Narrative presentation binding overflow")?;
        self.work = self
            .work
            .checked_add(work)
            .ok_or("Narrative presentation work overflow")?;
        self.check()
    }
    pub(crate) fn value(&mut self, value: &Value) -> Result<(), String> {
        self.work = self
            .work
            .checked_add(crate::value_limits::value_work(value).map_err(|error| error.to_string())?)
            .ok_or("Narrative value budget overflow")?;
        self.check()
    }
    pub(crate) fn binding(&mut self, name: &str, value: &Value) -> Result<(), String> {
        if name.is_empty() || name.len() > 4096 {
            return Err("Invalid narrative binding".into());
        }
        self.bindings = self
            .bindings
            .checked_add(1)
            .ok_or("Narrative binding overflow")?;
        self.value(value)
    }
    fn check(&self) -> Result<(), String> {
        if self.bindings > MAX_NARRATIVE_BINDINGS || self.work > MAX_NARRATIVE_VALUE_WORK {
            Err("Narrative state exceeds 4096 bindings or one million value-work units".into())
        } else {
            Ok(())
        }
    }

    pub(crate) fn values(&mut self, values: &HashMap<String, Value>) -> Result<(), String> {
        self.bindings = self
            .bindings
            .checked_add(values.len())
            .ok_or("Narrative binding overflow")?;
        for (name, value) in values {
            if name.is_empty() || name.len() > 4096 {
                return Err("Invalid narrative binding".into());
            }
            self.work = self
                .work
                .checked_add(
                    crate::value_limits::value_work(value).map_err(|error| error.to_string())?,
                )
                .ok_or("Narrative value budget overflow")?;
        }
        if self.bindings > MAX_NARRATIVE_BINDINGS || self.work > MAX_NARRATIVE_VALUE_WORK {
            Err("Narrative state exceeds 4096 bindings or one million value-work units".into())
        } else {
            Ok(())
        }
    }
}
