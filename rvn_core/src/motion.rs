//! Authoritative animation clocks: rendering and restore sample the same pose.
use rvn_ui::motion::{Motion, Pose};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MotionTarget {
    Background,
    Sprite { id: String },
    Layer { id: String, layer: String },
    Interface { screen: String, element: String },
}
impl MotionTarget {
    pub fn parse(text: &str) -> Result<Self, String> {
        let valid = |name: &str| {
            !name.is_empty()
                && name.len() <= 128
                && !name
                    .chars()
                    .any(|ch| ch.is_control() || matches!(ch, ':' | '/'))
        };
        if text == "background" {
            return Ok(Self::Background);
        }
        if let Some(id) = text.strip_prefix("sprite:").filter(|id| valid(id)) {
            return Ok(Self::Sprite { id: id.into() });
        }
        if let Some((id, layer)) = text
            .strip_prefix("layer:")
            .and_then(|rest| rest.split_once('/'))
            .filter(|(id, layer)| valid(id) && valid(layer))
        {
            return Ok(Self::Layer {
                id: id.into(),
                layer: layer.into(),
            });
        }
        if let Some((screen, element)) = text
            .strip_prefix("ui:")
            .and_then(|rest| rest.split_once('/'))
            .filter(|(screen, element)| valid(screen) && valid(element))
        {
            return Ok(Self::Interface {
                screen: screen.into(),
                element: element.into(),
            });
        }
        Err("Animation target: background, sprite:<character>, layer:<character>/<layer>, or ui:<screen>/<component> expected".into())
    }
    pub fn key(&self) -> String {
        match self {
            Self::Background => "background".into(),
            Self::Sprite { id } => format!("sprite:{id}"),
            Self::Layer { id, layer } => format!("layer:{id}/{layer}"),
            Self::Interface { screen, element } => format!("ui:{screen}/{element}"),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if Self::parse(&self.key())? == *self {
            Ok(())
        } else {
            Err("Invalid animation target".into())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub target: MotionTarget,
    pub definition: Motion,
    pub base: Pose,
    pub elapsed: f64,
    pub running: bool,
}
impl Track {
    pub fn validate(&self) -> Result<(), String> {
        self.target.validate()?;
        self.base.validate()?;
        let summary = self.definition.validate()?;
        if !self.elapsed.is_finite()
            || self.elapsed < 0.0
            || summary.seconds.is_some_and(|end| self.elapsed > end)
        {
            return Err("Invalid saved animation position".into());
        }
        if summary.seconds.is_none() && !self.running {
            return Err("An endless animation cannot be saved as completed".into());
        }
        if self.running && summary.seconds.is_some_and(|end| self.elapsed >= end) {
            return Err("A completed animation cannot be saved as running".into());
        }
        Ok(())
    }
    pub fn pose(&self) -> Result<Pose, String> {
        self.definition.sample(self.elapsed, &self.base)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionState {
    pub tracks: BTreeMap<String, Track>,
    pub waiting: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MotionView {
    pub target: MotionTarget,
    pub pose: Pose,
}
impl MotionState {
    pub fn validate(&self) -> Result<(), String> {
        if self.tracks.len() > 64 {
            return Err("Animation limit: 64 simultaneous targets".into());
        }
        for (key, track) in &self.tracks {
            track.validate()?;
            if *key != track.target.key() {
                return Err("Saved animation target identity mismatch".into());
            }
        }
        let resources: std::collections::BTreeSet<_> = self
            .tracks
            .values()
            .flat_map(|track| track.definition.frame_paths())
            .collect();
        if resources.len() > 4096 {
            return Err("Animation limit: 4096 simultaneous image resources".into());
        }
        if let Some(waiting) = &self.waiting {
            MotionTarget::parse(waiting)?;
            if self.tracks.get(waiting).is_some_and(|track| {
                track
                    .definition
                    .validate()
                    .is_ok_and(|summary| summary.seconds.is_none())
            }) {
                return Err("Cannot wait for an endless animation".into());
            }
        }
        Ok(())
    }
    pub fn views(&self) -> Result<Vec<MotionView>, String> {
        self.validate()?;
        self.tracks
            .values()
            .map(|track| {
                Ok(MotionView {
                    target: track.target.clone(),
                    pose: track.pose()?,
                })
            })
            .collect()
    }
    pub fn play(&mut self, target: MotionTarget, definition: Motion) -> Result<(), String> {
        let summary = definition.validate()?;
        let key = target.key();
        if !self.tracks.contains_key(&key) && self.tracks.len() >= 64 {
            return Err("Animation limit: 64 simultaneous targets".into());
        }
        let base = self
            .tracks
            .get(&key)
            .map(Track::pose)
            .transpose()?
            .unwrap_or_default();
        self.tracks.insert(
            key,
            Track {
                target,
                definition,
                base,
                elapsed: 0.0,
                running: summary.seconds != Some(0.0),
            },
        );
        Ok(())
    }
    pub fn tick(&mut self, seconds: f64) -> Result<(), String> {
        if !seconds.is_finite() || !(0.0..=3600.0).contains(&seconds) {
            return Err(
                "Animation clock delta must be finite and between 0 and 3600 seconds".into(),
            );
        }
        self.validate()?;
        for track in self.tracks.values_mut().filter(|track| track.running) {
            track.elapsed += seconds;
            if let Some(end) = track.definition.validate()?.seconds {
                if track.elapsed >= end {
                    track.elapsed = end;
                    track.running = false;
                }
            } else if track.elapsed > 1_000_000.0 {
                // Canonicalize an endless track without losing its cycle phase.
                if let Motion::Repeat { motion, .. } = &track.definition {
                    track.elapsed %= motion.validate()?.seconds.unwrap();
                }
            }
        }
        Ok(())
    }
}

/// Pure descriptor constructors, also exposed as ordinary Blueprint functions.
pub fn construct(
    name: &str,
    args: &[rvn_parser::Value],
) -> Result<rvn_parser::Value, crate::eval::EvalError> {
    use rvn_parser::Value;
    let invalid = |message: &str| crate::eval::EvalError::InvalidFunction(message.into());
    let keys: &[&str] = match name {
        "motion_tween" => &["seconds", "from", "to", "curve"],
        "motion_spline" => &["seconds", "points", "curve"],
        "motion_bezier" => &["x1", "y1", "x2", "y2"],
        "motion_sequence" | "motion_parallel" => &["steps"],
        "motion_pause" => &["seconds"],
        "motion_repeat" => &["times", "motion"],
        "motion_frames" => &["images", "fps"],
        _ => return Err(invalid("Unknown animation constructor")),
    };
    if args.len() != keys.len() {
        return Err(invalid("Animation constructor argument count mismatch"));
    }
    let mut description: BTreeMap<String, Value> = keys
        .iter()
        .zip(args)
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect();
    description.insert(
        "kind".into(),
        Value::Str(name.trim_start_matches("motion_").into()),
    );
    let value = Value::Dict(description);
    let json = crate::ui::value_to_json(&value)?;
    if name == "motion_bezier" {
        let curve: rvn_ui::motion::Easing = serde_json::from_value(json)
            .map_err(|problem| invalid(&format!("Invalid animation curve: {problem}")))?;
        curve.validate().map_err(|message| invalid(&message))?;
    } else {
        Motion::parse(json).map_err(|message| invalid(&message))?;
    }
    Ok(value)
}

/// Build a deterministic interpolation lookup table using the calling
/// computation's existing bounded function evaluator. A function is sampled
/// once; its captured result, rather than its source, is serialized in saves.
pub fn sample_curve(
    mut samples: usize,
    mut function: impl FnMut(f32) -> crate::eval::EvalResult<rvn_parser::Value>,
) -> crate::eval::EvalResult<rvn_parser::Value> {
    use crate::eval::EvalError;
    use rvn_parser::Value;
    if !(2..=257).contains(&samples) {
        return Err(EvalError::InvalidFunction(
            "A custom animation curve needs 2–257 samples".into(),
        ));
    }
    let mut values = Vec::with_capacity(samples);
    for index in 0..samples {
        let value = match function(index as f32 / (samples - 1) as f32)? {
            Value::Int(value) => value as f32,
            Value::Float(value) => value,
            _ => {
                return Err(EvalError::InvalidFunction(
                    "A custom animation curve must return a number".into(),
                ))
            }
        };
        values.push(value);
    }
    let curve = rvn_ui::motion::Easing::Samples {
        values: values.clone(),
    };
    curve.validate().map_err(EvalError::InvalidFunction)?;
    // Canonical endpoints eliminate floating point rounding from the user's
    // arithmetic while preserving all interior samples exactly.
    samples -= 1;
    values[0] = 0.0;
    values[samples] = 1.0;
    Ok(Value::Dict(BTreeMap::from([
        ("kind".into(), Value::Str("samples".into())),
        (
            "values".into(),
            Value::List(values.into_iter().map(Value::Float).collect()),
        ),
    ])))
}
