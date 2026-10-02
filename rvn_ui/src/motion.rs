//! Shared, seekable animation descriptions. Sampling never depends on frame
//! rate or renderer state, so preview, desktop, Web and restoration agree.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Cubic Bézier timing: x is time, y is progress. Unit-square controls
    /// guarantee a finite, seekable interpolation (including crossed x's).
    Bezier {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
    /// A pure RVN function is evaluated once when authored/played. The
    /// resulting uniformly spaced samples travel with the animation/save;
    /// no script callbacks or frame-dependent evaluation occur in a renderer.
    Samples {
        values: Vec<f32>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum EasingWire {
    Named(String),
    Custom(CustomEasing),
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CustomEasing {
    Bezier { x1: f32, y1: f32, x2: f32, y2: f32 },
    Samples { values: Vec<f32> },
}
impl Serialize for Easing {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let wire = match self {
            Self::Linear => EasingWire::Named("linear".into()),
            Self::EaseIn => EasingWire::Named("ease_in".into()),
            Self::EaseOut => EasingWire::Named("ease_out".into()),
            Self::EaseInOut => EasingWire::Named("ease_in_out".into()),
            Self::Bezier { x1, y1, x2, y2 } => EasingWire::Custom(CustomEasing::Bezier {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
            }),
            Self::Samples { values } => EasingWire::Custom(CustomEasing::Samples {
                values: values.clone(),
            }),
        };
        wire.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Easing {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match EasingWire::deserialize(deserializer)? {
            EasingWire::Named(name) => match name.as_str() {
                "linear" => Self::Linear,
                "ease_in" => Self::EaseIn,
                "ease_out" => Self::EaseOut,
                "ease_in_out" => Self::EaseInOut,
                _ => return Err(serde::de::Error::custom("Unknown animation easing")),
            },
            EasingWire::Custom(CustomEasing::Bezier { x1, y1, x2, y2 }) => {
                Self::Bezier { x1, y1, x2, y2 }
            }
            EasingWire::Custom(CustomEasing::Samples { values }) => Self::Samples { values },
        })
    }
}
impl Easing {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Bezier { x1, y1, x2, y2 } => {
                if [*x1, *y1, *x2, *y2]
                    .iter()
                    .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                {
                    return Err("Animation Bézier controls must be finite in [0,1]".into());
                }
            }
            Self::Samples { values } => {
                if !(2..=257).contains(&values.len())
                    || values
                        .iter()
                        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                    || values.first().is_none_or(|value| value.abs() > 0.00001)
                    || values
                        .last()
                        .is_none_or(|value| (*value - 1.0).abs() > 0.00001)
                {
                    return Err("A custom animation curve needs 2–257 finite samples in [0,1], starting at 0 and ending at 1".into());
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub fn sample(&self, value: f64) -> f32 {
        let t = value.clamp(0.0, 1.0) as f32;
        if t == 0.0 || t == 1.0 {
            return t;
        }
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => t * t * (3.0 - 2.0 * t),
            Self::Bezier { x1, y1, x2, y2 } => {
                if t == 0.0 || t == 1.0 {
                    return t;
                }
                let cubic = |u: f32, a: f32, b: f32| {
                    let v = 1.0 - u;
                    3.0 * v * v * u * a + 3.0 * v * u * u * b + u * u * u
                };
                // A fixed iteration count is deterministic, including vertical
                // tangents where Newton iteration would divide by zero.
                let (mut lo, mut hi) = (0.0, 1.0);
                for _ in 0..24 {
                    let mid = (lo + hi) * 0.5;
                    if cubic(mid, *x1, *x2) < t {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                cubic((lo + hi) * 0.5, *y1, *y2).clamp(0.0, 1.0)
            }
            Self::Samples { values } => {
                if values.len() < 2 {
                    return t;
                }
                let position = t * (values.len() - 1) as f32;
                let index = (position.floor() as usize).min(values.len() - 2);
                values[index] + (values[index + 1] - values[index]) * (position - index as f32)
            }
        }
    }
}

/// A Catmull–Rom trajectory passes through every authored point. Values are
/// offsets in the same reference pixels as tween x/y, not normalized UVs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathPoint {
    pub x: f32,
    pub y: f32,
}

/// Offsets are relative to the authored base in 1920×1080 reference pixels.
/// Rotation is clockwise in degrees. Pivot is normalized within the image.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pose {
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub tint_r: f32,
    pub tint_g: f32,
    pub tint_b: f32,
    pub tint_a: f32,
    pub pivot_x: f32,
    pub pivot_y: f32,
    pub frame: Option<String>,
}
impl Default for Pose {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            opacity: 1.0,
            tint_r: 1.0,
            tint_g: 1.0,
            tint_b: 1.0,
            tint_a: 1.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
            frame: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PosePatch {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub scale_x: Option<f32>,
    pub scale_y: Option<f32>,
    pub rotation: Option<f32>,
    pub opacity: Option<f32>,
    pub tint_r: Option<f32>,
    pub tint_g: Option<f32>,
    pub tint_b: Option<f32>,
    pub tint_a: Option<f32>,
    pub pivot_x: Option<f32>,
    pub pivot_y: Option<f32>,
}
impl PosePatch {
    fn values(&self) -> [Option<f32>; 12] {
        [
            self.x,
            self.y,
            self.scale_x,
            self.scale_y,
            self.rotation,
            self.opacity,
            self.tint_r,
            self.tint_g,
            self.tint_b,
            self.tint_a,
            self.pivot_x,
            self.pivot_y,
        ]
    }
    fn mask(&self) -> u16 {
        self.values()
            .iter()
            .enumerate()
            .fold(0, |mask, (index, value)| {
                mask | if value.is_some() { 1 << index } else { 0 }
            })
    }
    fn validate(&self) -> Result<(), String> {
        for (index, value) in self.values().into_iter().enumerate() {
            let Some(value) = value else { continue };
            let valid = value.is_finite()
                && match index {
                    0 | 1 | 4 => value.abs() <= 1_000_000.0,
                    2 | 3 => (0.001..=256.0).contains(&value),
                    _ => (0.0..=1.0).contains(&value),
                };
            if !valid {
                return Err(format!(
                    "Animation transform channel {index} is outside its finite range"
                ));
            }
        }
        Ok(())
    }
    fn apply(&self, pose: &mut Pose) {
        let values = self.values();
        let mut target = [
            &mut pose.x,
            &mut pose.y,
            &mut pose.scale_x,
            &mut pose.scale_y,
            &mut pose.rotation,
            &mut pose.opacity,
            &mut pose.tint_r,
            &mut pose.tint_g,
            &mut pose.tint_b,
            &mut pose.tint_a,
            &mut pose.pivot_x,
            &mut pose.pivot_y,
        ];
        for (value, target) in values.into_iter().zip(target.iter_mut()) {
            if let Some(value) = value {
                **target = value;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Motion {
    Tween {
        seconds: f64,
        #[serde(default)]
        from: PosePatch,
        to: PosePatch,
        #[serde(default)]
        curve: Easing,
    },
    Spline {
        seconds: f64,
        points: Vec<PathPoint>,
        #[serde(default)]
        curve: Easing,
    },
    Pause {
        seconds: f64,
    },
    Sequence {
        steps: Vec<Motion>,
    },
    Parallel {
        steps: Vec<Motion>,
    },
    Repeat {
        #[serde(deserialize_with = "repeat_count")]
        times: Option<u32>,
        motion: Box<Motion>,
    },
    Frames {
        images: Vec<String>,
        fps: f64,
    },
}

fn repeat_count<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u32>, D::Error> {
    Ok(Option::<u32>::deserialize(deserializer)?.filter(|times| *times != 0))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub seconds: Option<f64>,
    pub channels: u16,
}

impl Motion {
    /// All resources of a track, not merely its current frame. Renderers keep
    /// these handles alive across baseline restoration and preload transitions.
    pub fn frame_paths(&self) -> std::collections::BTreeSet<String> {
        fn visit(motion: &Motion, paths: &mut std::collections::BTreeSet<String>) {
            match motion {
                Motion::Frames { images, .. } => paths.extend(images.iter().cloned()),
                Motion::Sequence { steps } | Motion::Parallel { steps } => {
                    for step in steps {
                        visit(step, paths);
                    }
                }
                Motion::Repeat { motion, .. } => visit(motion, paths),
                _ => {}
            }
        }
        let mut paths = std::collections::BTreeSet::new();
        visit(self, &mut paths);
        paths
    }
    pub fn parse(description: serde_json::Value) -> Result<Self, String> {
        let motion: Self = serde_json::from_value(description)
            .map_err(|problem| format!("Invalid animation: {problem}"))?;
        motion.validate()?;
        Ok(motion)
    }
    pub fn validate(&self) -> Result<Summary, String> {
        fn seconds(value: f64) -> Result<f64, String> {
            if value.is_finite() && (0.0..=86400.0).contains(&value) {
                Ok(value)
            } else {
                Err("Animation duration must be finite, nonnegative and at most 24 hours".into())
            }
        }
        fn walk(motion: &Motion, depth: usize, count: &mut usize) -> Result<Summary, String> {
            *count += 1;
            if depth > 32 || *count > 512 {
                return Err("Animation limit: 512 operations and 32 nesting levels".into());
            }
            let result = match motion {
                Motion::Tween {
                    seconds: duration,
                    from,
                    to,
                    curve,
                } => {
                    from.validate()?;
                    to.validate()?;
                    curve.validate()?;
                    Summary {
                        seconds: Some(seconds(*duration)?),
                        channels: from.mask() | to.mask(),
                    }
                }
                Motion::Spline {
                    seconds: duration,
                    points,
                    curve,
                } => {
                    if !(2..=256).contains(&points.len())
                        || points.iter().any(|point| {
                            !point.x.is_finite()
                                || !point.y.is_finite()
                                || point.x.abs() > 500_000.0
                                || point.y.abs() > 500_000.0
                        })
                    {
                        return Err("An animation spline needs 2–256 finite points in ±500000 reference pixels".into());
                    }
                    curve.validate()?;
                    Summary {
                        seconds: Some(seconds(*duration)?),
                        channels: 3,
                    }
                }
                Motion::Pause { seconds: duration } => Summary {
                    seconds: Some(seconds(*duration)?),
                    channels: 0,
                },
                Motion::Frames { images, fps } => {
                    if images.is_empty()
                        || images.len() > 4096
                        || images
                            .iter()
                            .any(|path| !crate::programmable::safe_asset_path(path))
                    {
                        return Err(
                            "An animation frame list needs 1–4096 project-relative image resources"
                                .into(),
                        );
                    }
                    if !fps.is_finite() || !(0.01..=240.0).contains(fps) {
                        return Err("Animation frame rate must be between 0.01 and 240 fps".into());
                    }
                    Summary {
                        seconds: Some(seconds(images.len() as f64 / fps)?),
                        channels: 1 << 12,
                    }
                }
                Motion::Sequence { steps } | Motion::Parallel { steps } => {
                    if steps.is_empty() {
                        return Err("Animation sequence/parallel group must not be empty".into());
                    }
                    let parallel = matches!(motion, Motion::Parallel { .. });
                    let mut result = Summary {
                        seconds: Some(0.0),
                        channels: 0,
                    };
                    for (index, step) in steps.iter().enumerate() {
                        let child = walk(step, depth + 1, count)?;
                        if parallel && (result.channels & child.channels) != 0 {
                            return Err(
                                "Parallel animation branches write the same transform channel"
                                    .into(),
                            );
                        }
                        if !parallel && result.seconds.is_none() {
                            return Err(format!("Animation sequence operation {index} is unreachable after an endless repeat"));
                        }
                        result.channels |= child.channels;
                        result.seconds = match (result.seconds, child.seconds) {
                            (Some(a), Some(b)) => Some(if parallel { a.max(b) } else { a + b }),
                            _ => None,
                        };
                    }
                    result
                }
                Motion::Repeat { times, motion } => {
                    let child = walk(motion, depth + 1, count)?;
                    if times.is_some_and(|times| times == 0 || times > 1_000_000) {
                        return Err(
                            "Animation repeat count must be 1–1000000, or null for endless".into(),
                        );
                    }
                    if child.seconds.is_none_or(|seconds| seconds <= 0.0) {
                        return Err(
                            "Repeated animation must have a finite, positive cycle duration".into(),
                        );
                    }
                    Summary {
                        seconds: times.map(|times| child.seconds.unwrap() * f64::from(times)),
                        channels: child.channels,
                    }
                }
            };
            if let Some(duration) = result.seconds {
                seconds(duration)?;
            }
            Ok(result)
        }
        let summary = walk(self, 0, &mut 0)?;
        if self.frame_paths().len() > 4096 {
            return Err("Animation limit: 4096 unique image resources".into());
        }
        Ok(summary)
    }

    /// Seek a validated description. Finite out-of-range times clamp to the
    /// endpoints. Endless repetitions restart from the same base each cycle.
    /// No iteration proportional to elapsed time occurs.
    pub fn sample(&self, elapsed: f64, base: &Pose) -> Result<Pose, String> {
        if !elapsed.is_finite() {
            return Err("Animation seek time must be finite".into());
        }
        base.validate()?;
        self.validate()?;
        Ok(self.sample_valid(elapsed.max(0.0), base))
    }
    fn duration(&self) -> Option<f64> {
        match self {
            Self::Tween { seconds, .. }
            | Self::Spline { seconds, .. }
            | Self::Pause { seconds } => Some(*seconds),
            Self::Frames { images, fps } => Some(images.len() as f64 / fps),
            Self::Sequence { steps } => steps.iter().try_fold(0.0, |sum, step| {
                step.duration().map(|duration| sum + duration)
            }),
            Self::Parallel { steps } => steps.iter().try_fold(0.0f64, |longest, step| {
                step.duration().map(|duration| longest.max(duration))
            }),
            Self::Repeat { times, motion } => times.and_then(|times| {
                motion
                    .duration()
                    .map(|duration| duration * f64::from(times))
            }),
        }
    }
    fn channels(&self) -> u16 {
        match self {
            Self::Tween { from, to, .. } => from.mask() | to.mask(),
            Self::Spline { .. } => 3,
            Self::Pause { .. } => 0,
            Self::Frames { .. } => 1 << 12,
            Self::Sequence { steps } | Self::Parallel { steps } => {
                steps.iter().fold(0, |mask, step| mask | step.channels())
            }
            Self::Repeat { motion, .. } => motion.channels(),
        }
    }
    fn sample_valid(&self, elapsed: f64, base: &Pose) -> Pose {
        match self {
            Self::Pause { .. } => base.clone(),
            Self::Tween {
                seconds,
                from,
                to,
                curve,
            } => {
                let mut start = base.clone();
                from.apply(&mut start);
                let mut end = start.clone();
                to.apply(&mut end);
                let t = if *seconds == 0.0 {
                    1.0
                } else {
                    curve.sample(elapsed / seconds)
                };
                let a = start.values();
                let b = end.values();
                let values = std::array::from_fn(|index| a[index] + (b[index] - a[index]) * t);
                Pose::from_values(values, base.frame.clone())
            }
            Self::Spline {
                seconds,
                points,
                curve,
            } => {
                let progress = if *seconds == 0.0 {
                    1.0
                } else {
                    curve.sample(elapsed / seconds)
                };
                let position = progress * (points.len() - 1) as f32;
                let index = (position.floor() as usize).min(points.len() - 2);
                let t = position - index as f32;
                let a = points[index.saturating_sub(1)];
                let b = points[index];
                let c = points[index + 1];
                let d = points[(index + 2).min(points.len() - 1)];
                let channel = |a: f32, b: f32, c: f32, d: f32| {
                    0.5 * ((2.0 * b)
                        + (-a + c) * t
                        + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
                        + (-a + 3.0 * b - 3.0 * c + d) * t * t * t)
                };
                let mut pose = base.clone();
                pose.x = channel(a.x, b.x, c.x, d.x);
                pose.y = channel(a.y, b.y, c.y, d.y);
                pose
            }
            Self::Frames { images, fps } => {
                let mut pose = base.clone();
                let index = ((elapsed * fps).floor() as usize).min(images.len() - 1);
                pose.frame = Some(images[index].clone());
                pose
            }
            Self::Sequence { steps } => {
                let mut pose = base.clone();
                let mut remaining = elapsed;
                for step in steps {
                    match step.duration() {
                        Some(duration) if remaining >= duration => {
                            pose = step.sample_valid(duration, &pose);
                            remaining -= duration;
                        }
                        _ => return step.sample_valid(remaining, &pose),
                    }
                }
                pose
            }
            Self::Parallel { steps } => {
                let mut pose = base.clone();
                for step in steps {
                    let sampled = step.sample_valid(elapsed, base);
                    let mask = step.channels();
                    let mut values = pose.values();
                    let sampled_values = sampled.values();
                    for index in 0..12 {
                        if mask & (1 << index) != 0 {
                            values[index] = sampled_values[index];
                        }
                    }
                    pose = Pose::from_values(
                        values,
                        if mask & (1 << 12) != 0 {
                            sampled.frame
                        } else {
                            pose.frame
                        },
                    );
                }
                pose
            }
            Self::Repeat { times, motion } => {
                let duration = motion.duration().unwrap();
                if times.is_some_and(|times| elapsed >= duration * f64::from(times)) {
                    motion.sample_valid(duration, base)
                } else {
                    motion.sample_valid(elapsed % duration, base)
                }
            }
        }
    }
}
impl Pose {
    fn values(&self) -> [f32; 12] {
        [
            self.x,
            self.y,
            self.scale_x,
            self.scale_y,
            self.rotation,
            self.opacity,
            self.tint_r,
            self.tint_g,
            self.tint_b,
            self.tint_a,
            self.pivot_x,
            self.pivot_y,
        ]
    }
    fn from_values(v: [f32; 12], frame: Option<String>) -> Self {
        Self {
            x: v[0],
            y: v[1],
            scale_x: v[2],
            scale_y: v[3],
            rotation: v[4],
            opacity: v[5],
            tint_r: v[6],
            tint_g: v[7],
            tint_b: v[8],
            tint_a: v[9],
            pivot_x: v[10],
            pivot_y: v[11],
            frame,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let v = self.values();
        PosePatch {
            x: Some(v[0]),
            y: Some(v[1]),
            scale_x: Some(v[2]),
            scale_y: Some(v[3]),
            rotation: Some(v[4]),
            opacity: Some(v[5]),
            tint_r: Some(v[6]),
            tint_g: Some(v[7]),
            tint_b: Some(v[8]),
            tint_a: Some(v[9]),
            pivot_x: Some(v[10]),
            pivot_y: Some(v[11]),
        }
        .validate()?;
        if self
            .frame
            .as_ref()
            .is_some_and(|path| !crate::programmable::safe_asset_path(path))
        {
            return Err("Animation frame must be a project-relative image".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tween(seconds: f64, to: PosePatch) -> Motion {
        Motion::Tween {
            seconds,
            from: PosePatch::default(),
            to,
            curve: Easing::Linear,
        }
    }
    #[test]
    fn sequence_parallel_and_seeking_share_captured_values() {
        let motion = Motion::Sequence {
            steps: vec![
                tween(
                    2.0,
                    PosePatch {
                        x: Some(100.0),
                        ..Default::default()
                    },
                ),
                Motion::Parallel {
                    steps: vec![
                        tween(
                            2.0,
                            PosePatch {
                                x: Some(300.0),
                                ..Default::default()
                            },
                        ),
                        tween(
                            1.0,
                            PosePatch {
                                opacity: Some(0.0),
                                ..Default::default()
                            },
                        ),
                    ],
                },
            ],
        };
        assert_eq!(motion.validate().unwrap().seconds, Some(4.0));
        let middle = motion.sample(3.0, &Pose::default()).unwrap();
        assert_eq!(middle.x, 200.0);
        assert_eq!(middle.opacity, 0.0);
        assert_eq!(
            motion.sample(0.0, &Pose::default()).unwrap(),
            Pose::default()
        );
        assert_eq!(motion.sample(100.0, &Pose::default()).unwrap().x, 300.0);
        assert_eq!(motion.sample(3.0, &Pose::default()).unwrap(), middle);
    }
    #[test]
    fn repeated_frames_are_bounded_and_serialize_for_restoration() {
        let motion = Motion::Repeat {
            times: None,
            motion: Box::new(Motion::Frames {
                images: vec!["body/one.png".into(), "body/two.png".into()],
                fps: 2.0,
            }),
        };
        assert_eq!(motion.validate().unwrap().seconds, None);
        assert_eq!(
            motion
                .sample(1_000_000_000.75, &Pose::default())
                .unwrap()
                .frame
                .as_deref(),
            Some("body/two.png")
        );
        assert_eq!(
            Motion::parse(serde_json::to_value(&motion).unwrap())
                .unwrap()
                .sample(0.75, &Pose::default())
                .unwrap(),
            motion.sample(0.75, &Pose::default()).unwrap()
        );
    }
    #[test]
    fn invalid_channels_resources_infinite_sequences_and_times_are_not_silent() {
        let bad = Motion::Parallel {
            steps: vec![
                tween(
                    1.0,
                    PosePatch {
                        x: Some(1.0),
                        ..Default::default()
                    },
                ),
                tween(
                    1.0,
                    PosePatch {
                        x: Some(2.0),
                        ..Default::default()
                    },
                ),
            ],
        };
        assert!(bad.validate().is_err());
        assert!(Motion::Repeat {
            times: None,
            motion: Box::new(Motion::Pause { seconds: 0.0 })
        }
        .validate()
        .is_err());
        assert!(Motion::Sequence {
            steps: vec![
                Motion::Repeat {
                    times: None,
                    motion: Box::new(Motion::Pause { seconds: 1.0 })
                },
                Motion::Pause { seconds: 1.0 }
            ]
        }
        .validate()
        .is_err());
        assert!(Motion::Frames {
            images: vec!["../secret.png".into()],
            fps: 2.0
        }
        .validate()
        .is_err());
        assert!(
            Motion::parse(serde_json::json!({"kind":"pause","seconds":1,"misspelled":true}))
                .is_err()
        );
        assert!(tween(
            1.0,
            PosePatch {
                opacity: Some(2.0),
                ..Default::default()
            }
        )
        .validate()
        .is_err());
        assert!(tween(1.0, PosePatch::default())
            .sample(f64::NAN, &Pose::default())
            .is_err());
    }
    #[test]
    fn curves_endpoints_and_zero_duration_are_deterministic() {
        for curve in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
        ] {
            assert_eq!(curve.sample(0.0), 0.0);
            assert_eq!(curve.sample(1.0), 1.0);
            assert!((0.0..=1.0).contains(&curve.sample(0.4)));
        }
        assert_eq!(
            tween(
                0.0,
                PosePatch {
                    x: Some(40.0),
                    ..Default::default()
                }
            )
            .sample(0.0, &Pose::default())
            .unwrap()
            .x,
            40.0
        );
    }
    #[test]
    fn advanced_curves_and_splines_seek_and_restore_identically() {
        let curves = [
            Easing::Bezier {
                x1: 0.25,
                y1: 0.0,
                x2: 0.75,
                y2: 1.0,
            },
            Easing::Samples {
                values: vec![0.0, 0.0625, 0.25, 0.5625, 1.0],
            },
        ];
        for curve in curves {
            curve.validate().unwrap();
            assert_eq!(curve.sample(0.0), 0.0);
            assert_eq!(curve.sample(1.0), 1.0);
            let motion = Motion::Spline {
                seconds: 2.0,
                points: vec![
                    PathPoint { x: 0.0, y: 0.0 },
                    PathPoint { x: 100.0, y: -50.0 },
                    PathPoint { x: 200.0, y: 0.0 },
                ],
                curve,
            };
            let restored = Motion::parse(serde_json::to_value(&motion).unwrap()).unwrap();
            for time in [0.0, 0.1, 0.7, 1.0, 1.9, 2.0, 100.0] {
                assert_eq!(
                    motion.sample(time, &Pose::default()),
                    restored.sample(time, &Pose::default())
                );
            }
            assert_eq!(motion.sample(0.0, &Pose::default()).unwrap().x, 0.0);
            assert_eq!(motion.sample(20.0, &Pose::default()).unwrap().x, 200.0);
        }
        let straight = Motion::Spline {
            seconds: 2.0,
            points: vec![
                PathPoint { x: 0.0, y: 0.0 },
                PathPoint { x: 100.0, y: -50.0 },
                PathPoint { x: 200.0, y: 0.0 },
            ],
            curve: Easing::Linear,
        };
        let mid = straight.sample(1.0, &Pose::default()).unwrap();
        assert_eq!((mid.x, mid.y), (100.0, -50.0));
        let combined = Motion::Parallel {
            steps: vec![
                straight.clone(),
                tween(
                    2.0,
                    PosePatch {
                        rotation: Some(180.0),
                        ..Default::default()
                    },
                ),
            ],
        };
        assert_eq!(
            combined.sample(1.0, &Pose::default()).unwrap().rotation,
            90.0
        );
        assert!(Motion::Parallel {
            steps: vec![
                straight,
                tween(
                    1.0,
                    PosePatch {
                        x: Some(10.0),
                        ..Default::default()
                    }
                )
            ]
        }
        .validate()
        .is_err());
    }
    #[test]
    fn custom_curves_and_paths_reject_invalid_data_and_legacy_easing_stays_readable() {
        for json in [
            serde_json::json!({"kind":"samples","values":[0,2,1]}),
            serde_json::json!({"kind":"samples","values":[0.1,1]}),
            serde_json::json!({"kind":"bezier","x1":1.8,"y1":0,"x2":0.2,"y2":1}),
        ] {
            assert!(serde_json::from_value::<Easing>(json)
                .unwrap()
                .validate()
                .is_err());
        }
        assert!(Motion::parse(
            serde_json::json!({"kind":"spline","seconds":1,"points":[{"x":0,"y":0}]})
        )
        .is_err());
        for curve in ["linear", "ease_in", "ease_out", "ease_in_out"] {
            let motion=Motion::parse(serde_json::json!({"kind":"tween","seconds":1,"from":{},"to":{"x":1},"curve":curve})).unwrap();
            assert_eq!(serde_json::to_value(&motion).unwrap()["curve"], curve);
        }
    }
}
