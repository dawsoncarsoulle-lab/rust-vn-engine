//! PRIVATE STAGE: logical outgoing appearances; no Entity, handle or callback.
//! Installation requires the accompanying registry patch and Root command glue.
use crate::composition::CharacterComposition;
use crate::scene_resources::{
    EffectClock, EffectParameters, ResourceTaskKind, SavedEffect, SpriteEffectKind, TargetIdentity,
    TargetKind, TargetRecord, TransformSnapshot, VisualTransition,
};
use crate::scene_tasks::TaskOwner;
use crate::sprite_effects::SpriteStyle;
use crate::types::{GameState, SpriteState};
use rvn_ui::motion::Pose;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresentationSlot {
    Background,
    Cinematic,
    Sprite { id: String },
    Music,
}
impl PresentationSlot {
    pub fn source_address(&self) -> (TargetKind, &str) {
        match self {
            Self::Background => (TargetKind::Background, "background"),
            Self::Cinematic => (TargetKind::Cinematic, "cinematic"),
            Self::Sprite { id } => (TargetKind::Sprite, id),
            Self::Music => (TargetKind::Music, "music"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenSpriteSample {
    pub animation: SpriteEffectKind,
    pub clock: EffectClock,
    pub base: TransformSnapshot,
    pub base_resolved: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenFadeSample {
    pub from: f32,
    pub to: f32,
    pub transition: VisualTransition,
    pub clock: EffectClock,
    pub base: TransformSnapshot,
    pub base_resolved: bool,
    pub layered: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenAudioSample {
    pub from: f32,
    pub to: f32,
    pub clock: EffectClock,
}
impl FrozenAudioSample {
    pub fn gain(&self) -> Result<f32, String> {
        SavedEffect {
            presentation: None,
            base_resolved: false,
            clock: self.clock.clone(),
            parameters: EffectParameters::AudioFade {
                from: self.from,
                to: self.to,
                despawn_on_done: false,
            },
        }
        .validate()?;
        let t = if self.clock.duration <= 0.0 {
            1.0
        } else {
            (self.clock.elapsed / self.clock.duration).clamp(0.0, 1.0)
        };
        // Same f64 absolute interpolation as C10 Current Audio, rounded once
        // to the sink's f32 gain; Music.volume is not multiplied again.
        Ok((f64::from(self.from) + (f64::from(self.to) - f64::from(self.from)) * t) as f32)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenMotionSample {
    pub pose: Pose,
    pub reduced_pose: Pose,
}
impl FrozenMotionSample {
    pub fn selected(&self, reduced_motion: bool) -> &Pose {
        if reduced_motion {
            &self.reduced_pose
        } else {
            &self.pose
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.pose.validate()?;
        self.reduced_pose.validate()
    }
    pub fn frame_paths(&self) -> impl Iterator<Item = &str> {
        [
            self.pose.frame.as_deref(),
            self.reduced_pose.frame.as_deref(),
        ]
        .into_iter()
        .flatten()
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenLayerSample {
    pub original_target: TargetRecord,
    pub sprite: Option<FrozenSpriteSample>,
    pub fade: Option<FrozenFadeSample>,
    pub motion: Option<FrozenMotionSample>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenVisualSample {
    pub sprite: Option<FrozenSpriteSample>,
    pub fade: Option<FrozenFadeSample>,
    pub root_motion: Option<FrozenMotionSample>,
    #[serde(default)]
    pub layers: BTreeMap<String, FrozenLayerSample>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RetiredModel {
    /// Logical catalogue ID; resolved without looking up the Current object.
    Cinematic {
        id: String,
        sample: FrozenVisualSample,
    },
    Background {
        path: String,
        sample: FrozenVisualSample,
    },
    Sprite {
        id: String,
        state: SpriteState,
        style: SpriteStyle,
        composition: Option<CharacterComposition>,
        sample: FrozenVisualSample,
    },
    Music {
        file: String,
        volume: f32,
        fade: Option<FrozenAudioSample>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetiredPresentation {
    pub original_slot: PresentationSlot,
    // Historical provenance, deliberately NOT a live dependency.
    pub original_target: TargetRecord,
    pub model: RetiredModel,
}

fn asset(path: &str) -> Result<(), String> {
    if rvn_ui::programmable::safe_asset_path(path) {
        Ok(())
    } else {
        Err("Retired appearance has an invalid project-relative asset".into())
    }
}
fn sprite_path(id: &str, sprite: &SpriteState) -> String {
    match &sprite.emotion {
        Some(path) if path.contains('/') => path.strip_prefix("assets/").unwrap_or(path).into(),
        Some(emotion) => format!("sprites/{id}/{emotion}.png"),
        None => format!("sprites/{id}/default.png"),
    }
}
fn validate_source_sample(
    sprite: &Option<FrozenSpriteSample>,
    fade: &Option<FrozenFadeSample>,
) -> Result<(), String> {
    if let Some(sample) = sprite {
        SavedEffect {
            presentation: None,
            base_resolved: sample.base_resolved,
            clock: sample.clock.clone(),
            parameters: EffectParameters::SpriteAnimation {
                animation: sample.animation.clone(),
                base: sample.base.clone(),
            },
        }
        .validate()?;
    }
    if let Some(sample) = fade {
        SavedEffect {
            presentation: None,
            base_resolved: sample.base_resolved,
            clock: sample.clock.clone(),
            parameters: EffectParameters::VisualFade {
                from: sample.from,
                to: sample.to,
                transition: sample.transition,
                base: sample.base.clone(),
                layered: sample.layered,
                despawn_on_finish: false,
            },
        }
        .validate()?;
    }
    Ok(())
}
impl FrozenVisualSample {
    fn validate(
        &self,
        sprite_id: Option<&str>,
        composition: Option<&CharacterComposition>,
    ) -> Result<(), String> {
        validate_source_sample(&self.sprite, &self.fade)?;
        if let Some(pose) = &self.root_motion {
            pose.validate()?;
        }
        if self.layers.len() > 128 {
            return Err("Retired sample exceeds 128 layers".into());
        }
        for (id, sample) in &self.layers {
            let character = sprite_id.ok_or("Only a composed sprite can retain layer samples")?;
            let composition = composition.ok_or("Retired layer sample has no composition")?;
            if !composition
                .definition
                .visible_layers(&composition.attributes)
                .any(|layer| layer.id == *id)
                || sample.original_target.owner != TaskOwner::Game
                || sample.original_target.identity.kind != TargetKind::Layer
                || sample.original_target.identity.alias != format!("{character}/{id}")
                || sample.original_target.identity.incarnation == 0
            {
                return Err("Retired layer sample is not an original selected layer".into());
            }
            validate_source_sample(&sample.sprite, &sample.fade)?;
            if sample.fade.as_ref().is_some_and(|fade| !fade.layered) {
                return Err("Historical layer fade has the wrong target family".into());
            }
            if let Some(pose) = &sample.motion {
                pose.validate()?;
            }
        }
        Ok(())
    }
    fn frame_paths(&self) -> impl Iterator<Item = &str> {
        self.root_motion
            .iter()
            .flat_map(FrozenMotionSample::frame_paths)
            .chain(
                self.layers
                    .values()
                    .filter_map(|sample| sample.motion.as_ref())
                    .flat_map(FrozenMotionSample::frame_paths),
            )
    }
}
impl RetiredPresentation {
    pub fn validate_structure(&self) -> Result<(), String> {
        let (kind, alias) = self.original_slot.source_address();
        if self.original_target.owner != TaskOwner::Game
            || self.original_target.identity.kind != kind
            || self.original_target.identity.alias != alias
            || self.original_target.identity.incarnation == 0
        {
            return Err("Retired source slot and historical identity disagree".into());
        }
        match (&self.original_slot, &self.model) {
            (PresentationSlot::Cinematic, RetiredModel::Cinematic { id, sample }) => {
                asset(id)?;
                if sample.sprite.is_some() || sample.root_motion.is_some() || !sample.layers.is_empty()
                    || sample.fade.as_ref().is_some_and(|fade|fade.layered) {
                    return Err("CG can retain only its exact visual fade".into());
                }
                sample.validate(None, None)?;
            }
            (PresentationSlot::Background, RetiredModel::Background { path, sample }) => {
                asset(path)?;
                if sample.sprite.is_some()
                    || !sample.layers.is_empty()
                    || sample.fade.as_ref().is_some_and(|fade| fade.layered)
                {
                    return Err("Background cannot contain sprite/layer samples".into());
                }
                sample.validate(None, None)?;
            }
            (
                PresentationSlot::Sprite { id: slot_id },
                RetiredModel::Sprite {
                    id,
                    state,
                    style,
                    composition,
                    sample,
                },
            ) => {
                if id != slot_id || !rvn_ui::composition::valid_name(id) || !state.visible {
                    return Err("Retired sprite must be the original visible character".into());
                }
                if matches!(state.position, rvn_parser::Position::Custom(x) if !x.is_finite()) {
                    return Err("Invalid retired sprite position".into());
                }
                style.validate()?;
                if let Some(composition) = composition {
                    composition.definition.validate()?;
                    composition
                        .definition
                        .validate_attributes(&composition.attributes)?;
                } else {
                    asset(&sprite_path(id, state))?;
                }
                sample.validate(Some(id), composition.as_ref())?;
            }
            (PresentationSlot::Music, RetiredModel::Music { file, volume, fade }) => {
                asset(file)?;
                if !volume.is_finite() || !(0.0..=1.0).contains(volume) {
                    return Err("Invalid retired music volume".into());
                }
                if let Some(sample) = fade {
                    SavedEffect {
                        presentation: None,
                        base_resolved: false,
                        clock: sample.clock.clone(),
                        parameters: EffectParameters::AudioFade {
                            from: sample.from,
                            to: sample.to,
                            despawn_on_done: false,
                        },
                    }
                    .validate()?;
                }
            }
            _ => return Err("Retired model and typed slot disagree".into()),
        }
        for path in self.resource_paths() {
            asset(&path)?;
        }
        Ok(())
    }
    pub fn validate_for_target(
        &self,
        retired: &TargetIdentity,
        next_incarnation: u64,
    ) -> Result<(), String> {
        self.validate_structure()?;
        if retired.kind != TargetKind::PresentationRetired
            || self.original_target.identity.incarnation >= retired.incarnation
            || retired.incarnation > next_incarnation
        {
            return Err("Retired historical source is not older than its outgoing identity".into());
        }
        let layers = match &self.model {
            RetiredModel::Sprite { sample, .. } => Some(&sample.layers),
            _ => None,
        };
        if layers.is_some_and(|layers| {
            layers
                .values()
                .any(|sample| sample.original_target.identity.incarnation >= retired.incarnation)
        }) {
            return Err("Retired layer identity is not historical".into());
        }
        let mut historical = BTreeSet::from([self.original_target.identity.incarnation]);
        if layers.is_some_and(|layers| {
            layers
                .values()
                .any(|sample| !historical.insert(sample.original_target.identity.incarnation))
        }) {
            return Err("Historical source incarnations are duplicated".into());
        }
        Ok(())
    }
    pub fn validate_outgoing(&self, effect: &SavedEffect) -> Result<(), String> {
        self.validate_structure()?;
        if effect.clock.looping {
            return Err("Retired presentations cannot loop".into());
        }
        let valid = match (&self.model, &effect.parameters) {
            (
                RetiredModel::Music { .. },
                EffectParameters::AudioFade {
                    from,
                    to,
                    despawn_on_done,
                    ..
                },
            ) => {
                from.to_bits() == 1.0f32.to_bits()
                    && to.to_bits() == 0.0f32.to_bits()
                    && *despawn_on_done
            }
            (
                RetiredModel::Background { .. } | RetiredModel::Cinematic { .. },
                EffectParameters::VisualFade {
                    from,
                    to,
                    despawn_on_finish,
                    layered,
                    ..
                },
            ) => {
                from.to_bits() == 1.0f32.to_bits()
                    && to.to_bits() == 0.0f32.to_bits()
                    && *despawn_on_finish
                    && !*layered
            }
            (
                RetiredModel::Sprite { composition, .. },
                EffectParameters::VisualFade {
                    from,
                    to,
                    despawn_on_finish,
                    layered,
                    ..
                },
            ) => {
                from.to_bits() == 1.0f32.to_bits()
                    && to.to_bits() == 0.0f32.to_bits()
                    && *despawn_on_finish
                    && *layered == composition.is_some()
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err("Retired appearance requires its matching outgoing fade family".into())
        }
    }
    pub fn outgoing_music_gain(&self, effect: &SavedEffect) -> Result<Option<f32>, String> {
        if !effect
            .presentation
            .as_ref()
            .is_some_and(|payload| payload.bit_exact(self))
        {
            return Err("Retired gain must sample its exact saved payload".into());
        }
        self.validate_outgoing(effect)?;
        effect.clock.validate()?;
        let RetiredModel::Music { volume, fade, .. } = &self.model else {
            return Ok(None);
        };
        let source_gain = match fade {
            Some(sample) => sample.gain()?,
            None => *volume,
        };
        let t = if effect.clock.duration <= 0.0 {
            1.0
        } else {
            (effect.clock.elapsed / effect.clock.duration).clamp(0.0, 1.0)
        };
        Ok(Some((f64::from(source_gain) * (1.0 - t)) as f32))
    }
    pub fn resource_paths(&self) -> BTreeSet<String> {
        let mut paths = BTreeSet::new();
        let sample = match &self.model {
            RetiredModel::Cinematic { id, sample } => {
                paths.insert(id.clone());
                Some(sample)
            }
            RetiredModel::Background { path, sample } => {
                paths.insert(path.clone());
                Some(sample)
            }
            RetiredModel::Sprite {
                id,
                state,
                composition,
                sample,
                ..
            } => {
                if let Some(composition) = composition {
                    paths.extend(composition.definition.image_paths());
                } else {
                    paths.insert(sprite_path(id, state));
                }
                Some(sample)
            }
            RetiredModel::Music { file, .. } => {
                paths.insert(file.clone());
                None
            }
        };
        if let Some(sample) = sample {
            paths.extend(sample.frame_paths().map(str::to_owned));
        }
        paths
    }
    pub(crate) fn budget(&self, values: &mut crate::scene::NarrativeBudget) -> Result<(), String> {
        let mut bindings = 1;
        if let RetiredModel::Sprite {
            composition,
            sample,
            ..
        } = &self.model
        {
            bindings += sample.layers.len();
            if let Some(composition) = composition {
                bindings += composition.attributes.len() + composition.definition.defaults.len();
                bindings += composition.definition.options.variants.len()
                    + composition.definition.options.rules.len();
                for layer in &composition.definition.layers {
                    bindings += 1 + layer.when.len() + layer.unless.len();
                }
                for attributes in composition.definition.options.variants.values() {
                    bindings += attributes.len();
                }
                for rule in composition.definition.options.rules.values() {
                    bindings += rule.when.len() + rule.unless.len() + rule.set.len();
                }
            }
        }
        let bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        values.presentation(bindings, bytes.len())
    }
    pub(crate) fn bit_exact(&self, other: &Self) -> bool {
        // All fields are deterministic BTree collections or structured values.
        // Validation rejects non-finite numbers before capture comparisons.
        match (serde_json::to_vec(self), serde_json::to_vec(other)) {
            (Ok(left), Ok(right)) => left == right,
            _ => false,
        }
    }
}

fn sample_for_target(
    state: &GameState,
    target: &TargetIdentity,
) -> Result<
    (
        Option<FrozenSpriteSample>,
        Option<FrozenFadeSample>,
        Option<FrozenMotionSample>,
    ),
    String,
> {
    let execution = state
        .scene_execution
        .as_ref()
        .ok_or("Retired source requires an explicit narrative program")?;
    let mut sprite = None;
    let mut fade = None;
    for task in execution
        .resources
        .active_effects(execution.owner())
        .into_iter()
        .filter(|task| task.target == *target)
    {
        let effect = task
            .effect
            .as_ref()
            .ok_or("Source effect descriptor is missing")?;
        match &effect.parameters {
            EffectParameters::SpriteAnimation { animation, base } => {
                if sprite.is_some() {
                    return Err("Ambiguous sprite effect source sample".into());
                }
                sprite = Some(FrozenSpriteSample {
                    animation: animation.clone(),
                    clock: effect.clock.clone(),
                    base: base.clone(),
                    base_resolved: effect.base_resolved,
                });
            }
            EffectParameters::VisualFade {
                from,
                to,
                transition,
                base,
                layered,
                ..
            } => {
                if fade.is_some() {
                    return Err("Ambiguous visual fade source sample".into());
                }
                fade = Some(FrozenFadeSample {
                    from: *from,
                    to: *to,
                    transition: *transition,
                    base: base.clone(),
                    layered: *layered,
                    clock: effect.clock.clone(),
                    base_resolved: effect.base_resolved,
                });
            }
            EffectParameters::AudioFade { .. } => {}
        }
    }
    let mut motion = None;
    for (alias, track) in &state.motions.tracks {
        if let Some(task) = execution
            .resources
            .task(ResourceTaskKind::Motion, alias, execution.owner())
            .filter(|task| task.target == *target)
        {
            if motion.is_some() {
                return Err("Ambiguous source motion pose".into());
            }
            // The current descriptor and this exact task were validated together.
            if task.parked {
                return Err("Parked source motion cannot become a visible sample".into());
            }
            let pose = track.pose()?;
            let reduced_pose = if track.running {
                track.base.clone()
            } else {
                pose.clone()
            };
            let sample = FrozenMotionSample { pose, reduced_pose };
            sample.validate()?;
            motion = Some(sample);
        }
    }
    Ok((sprite, fade, motion))
}
pub fn snapshot_retired_source(
    state: &GameState,
    slot: &PresentationSlot,
) -> Result<Option<RetiredPresentation>, String> {
    crate::scene_resources::validate_current_resource_state(state)?;
    if state.scene_execution.is_none() && matches!(slot,PresentationSlot::Music){return crate::game_audio::snapshot_music(state);}
    let execution = state
        .scene_execution
        .as_ref()
        .ok_or("Retired source requires an explicit narrative program")?;
    let absent = match slot {
        PresentationSlot::Background => state.background_image.is_empty(),
        PresentationSlot::Cinematic => state.cinematic.current.is_none(),
        PresentationSlot::Sprite { id } => {
            !state.sprites.get(id).is_some_and(|sprite| sprite.visible)
        }
        PresentationSlot::Music => state.music.current_file.is_none(),
    };
    if absent {
        return Ok(None);
    }
    let (kind, alias) = slot.source_address();
    let original_target = execution
        .resources
        .targets
        .iter()
        .find(|record| {
            record.owner == TaskOwner::Game
                && record.identity.kind == kind
                && record.identity.alias == alias
        })
        .ok_or("Current appearance has no exact Game incarnation")?
        .clone();
    let (sprite, fade, root_motion) = sample_for_target(state, &original_target.identity)?;
    let mut sample = FrozenVisualSample {
        sprite,
        fade,
        root_motion,
        layers: BTreeMap::new(),
    };
    let model = match slot {
        PresentationSlot::Cinematic => RetiredModel::Cinematic {
            id: state.cinematic.current.clone().ok_or("Missing current CG")?,
            sample,
        },
        PresentationSlot::Background => RetiredModel::Background {
            path: state.background_image.clone(),
            sample,
        },
        PresentationSlot::Sprite { id } => {
            let composition = state.layered.characters.get(id).cloned();
            if let Some(composition) = &composition {
                for layer in composition
                    .definition
                    .visible_layers(&composition.attributes)
                {
                    let alias = format!("{id}/{}", layer.id);
                    let layer_target = execution
                        .resources
                        .targets
                        .iter()
                        .find(|record| {
                            record.owner == TaskOwner::Game
                                && record.identity.kind == TargetKind::Layer
                                && record.identity.alias == alias
                        })
                        .ok_or("Selected source layer has no exact Game incarnation")?
                        .clone();
                    let (sprite, fade, motion) = sample_for_target(state, &layer_target.identity)?;
                    if sprite.is_some() || fade.is_some() || motion.is_some() {
                        sample.layers.insert(
                            layer.id.clone(),
                            FrozenLayerSample {
                                original_target: layer_target,
                                sprite,
                                fade,
                                motion,
                            },
                        );
                    }
                }
            }
            RetiredModel::Sprite {
                id: id.clone(),
                state: state.sprites[id].clone(),
                style: execution.sprite_styles.get(id).cloned().unwrap_or_default(),
                composition,
                sample,
            }
        }
        PresentationSlot::Music => {
            let mut fade = None;
            for task in execution
                .resources
                .active_effects(execution.owner())
                .into_iter()
                .filter(|task| task.target == original_target.identity)
            {
                if let Some(SavedEffect {
                    clock,
                    parameters: EffectParameters::AudioFade { from, to, .. },
                    ..
                }) = &task.effect
                {
                    if fade.is_some() {
                        return Err("Ambiguous source audio fade".into());
                    }
                    fade = Some(FrozenAudioSample {
                        from: *from,
                        to: *to,
                        clock: clock.clone(),
                    });
                }
            }
            RetiredModel::Music {
                file: state.music.current_file.clone().unwrap(),
                volume: state.music.volume,
                fade,
            }
        }
    };
    let captured = RetiredPresentation {
        original_slot: slot.clone(),
        original_target,
        model,
    };
    captured.validate_structure()?;
    Ok(Some(captured))
}
