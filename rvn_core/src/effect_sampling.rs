//! PRIVATE C11 stage. Pure extraction of existing Rust presentation math.
//! Inputs come from validated logical descriptions; no ECS clock is advanced.
//! Legacy f32 adapters retain their existing functions. Bevy conversion and
//! equivalence tests must be integrated before this module is published.
use crate::scene_resources::{EffectClock, SpriteEffectKind, TransformSnapshot, VisualTransition};
use rvn_ui::motion::Pose;

pub fn progress(clock: &EffectClock) -> f64 {
    if clock.duration <= 0.0 {
        1.0
    } else {
        (clock.elapsed / clock.duration).clamp(0.0, 1.0)
    }
}

pub fn sample_sprite(
    animation: &SpriteEffectKind,
    clock: &EffectClock,
    neutral: &TransformSnapshot,
    reduced_motion: bool,
) -> TransformSnapshot {
    let mut pose = neutral.clone();
    if reduced_motion || !clock.looping && clock.elapsed >= clock.duration {
        return pose;
    }
    let raw = if clock.duration > 0.0 {
        clock.elapsed / clock.duration
    } else {
        1.0
    };
    let phase = if clock.looping {
        raw.fract()
    } else {
        raw.min(1.0)
    };
    match *animation {
        SpriteEffectKind::Shake { intensity } => {
            let decay = if clock.looping { 1.0 } else { 1.0 - phase };
            pose.translation[0] +=
                ((phase * std::f64::consts::TAU * 6.0).sin() * f64::from(intensity) * decay) as f32;
        }
        SpriteEffectKind::Bounce { height } => {
            pose.translation[1] +=
                ((phase * std::f64::consts::PI).sin() * f64::from(height)) as f32;
        }
        SpriteEffectKind::Pulse { scale } => {
            let factor = (1.0
                + (f64::from(scale) - 1.0) * (phase * std::f64::consts::TAU).sin().max(0.0))
                as f32;
            for value in &mut pose.scale {
                *value *= factor;
            }
        }
    }
    pose
}

#[derive(Clone, Debug, PartialEq)]
pub struct SampledVisual {
    pub transform: TransformSnapshot,
    /// Keep the factor f64 until the adapter combines original tint/layer alpha.
    /// The existing layered path deliberately rounds its factor before tint.
    pub alpha: f64,
}
impl SampledVisual {
    pub fn with_original_alpha(&self, original: f32) -> f32 {
        (f64::from(original) * self.alpha) as f32
    }
}

pub fn sample_visual(
    transition: VisualTransition,
    from: f32,
    to: f32,
    clock: &EffectClock,
    neutral_or_animated: &TransformSnapshot,
    reduced_motion: bool,
) -> SampledVisual {
    let phase = if reduced_motion { 1.0 } else { progress(clock) };
    sample_visual_at(transition, from, to, neutral_or_animated, phase)
}

/// `slide_distance` remains the existing Rust renderer's 1920 policy. This is
/// not an Unreal measurement; a later renderer policy change is separate work.
pub fn sample_visual_at(
    transition: VisualTransition,
    from: f32,
    to: f32,
    neutral_or_animated: &TransformSnapshot,
    phase: f64,
) -> SampledVisual {
    let mut transform = neutral_or_animated.clone();
    let displacement = if to > from { 1.0 - phase } else { phase };
    match transition {
        VisualTransition::SlideLeft => transform.translation[0] += (1920.0 * displacement) as f32,
        VisualTransition::SlideRight => transform.translation[0] -= (1920.0 * displacement) as f32,
        VisualTransition::SlideUp => transform.translation[1] -= (1920.0 * displacement) as f32,
        VisualTransition::SlideDown => transform.translation[1] += (1920.0 * displacement) as f32,
        VisualTransition::ZoomIn => {
            for value in &mut transform.scale {
                *value *= (1.0 - 0.5 * displacement) as f32;
            }
        }
        VisualTransition::ZoomOut => {
            for value in &mut transform.scale {
                *value *= (1.0 + 0.5 * displacement) as f32;
            }
        }
        _ => {}
    }
    SampledVisual {
        transform,
        alpha: interpolation(from, to, phase),
    }
}

/// Current music uses absolute gains. Retired music uses this function for a
/// 1→0 factor, multiplied once by the separately frozen absolute source gain.
pub fn sample_audio(from: f32, to: f32, clock: &EffectClock) -> f32 {
    sample_audio_at(from, to, progress(clock))
}
fn sample_audio_at(from: f32, to: f32, phase: f64) -> f32 {
    interpolation(from, to, phase) as f32
}
fn interpolation(from: f32, to: f32, phase: f64) -> f64 {
    f64::from(from) + (f64::from(to) - f64::from(from)) * phase
}
pub fn retired_music_gain(frozen_absolute_gain: f32, clock: &EffectClock) -> f32 {
    (f64::from(frozen_absolute_gain) * (1.0 - progress(clock))) as f32
}

#[derive(Clone, Copy, Debug)]
pub struct MotionGeometry {
    pub size: [f32; 2],
    pub factor: [f32; 2],
    pub ui: bool,
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn rotate(q: [f32; 4], vector: [f32; 3]) -> [f32; 3] {
    let axis = [q[0], q[1], q[2]];
    // The saved base permits a small norm tolerance. Use the general formula,
    // rather than an algebraic shortcut that assumes norm squared is exactly 1.
    let norm_axis = axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2];
    let dot = axis[0] * vector[0] + axis[1] * vector[1] + axis[2] * vector[2];
    let perpendicular = cross(axis, vector);
    std::array::from_fn(|i| {
        vector[i] * (q[3] * q[3] - norm_axis)
            + axis[i] * (2.0 * dot)
            + perpendicular[i] * (2.0 * q[3])
    })
}
fn quaternion_product(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// Same existing Motion ordering: translation, local pivot, rotation and scale
/// relative to the base already sampled by Sprite/Fade. Geometry is explicit.
pub fn compose_motion(
    base: &TransformSnapshot,
    pose: &Pose,
    geometry: MotionGeometry,
) -> TransformSnapshot {
    let sign = if geometry.ui { 1.0 } else { -1.0 };
    let half_angle = sign * pose.rotation.to_radians() * 0.5;
    let rotation = [0.0, 0.0, half_angle.sin(), half_angle.cos()];
    let scale = [pose.scale_x, pose.scale_y, 1.0];
    let pivot = [
        (pose.pivot_x - 0.5) * geometry.size[0] * base.scale[0],
        sign * (pose.pivot_y - 0.5) * geometry.size[1] * base.scale[1],
        0.0,
    ];
    let rotated = rotate(rotation, std::array::from_fn(|i| scale[i] * pivot[i]));
    let offset = rotate(
        base.rotation,
        std::array::from_fn(|i| pivot[i] - rotated[i]),
    );
    let translation = [
        pose.x * geometry.factor[0],
        sign * pose.y * geometry.factor[1],
        0.0,
    ];
    TransformSnapshot {
        translation: std::array::from_fn(|i| base.translation[i] + translation[i] + offset[i]),
        scale: std::array::from_fn(|i| base.scale[i] * scale[i]),
        rotation: quaternion_product(base.rotation, rotation),
    }
}

pub fn motion_color(original_srgba: [f32; 4], pose: &Pose) -> [f32; 4] {
    [
        original_srgba[0] * pose.tint_r,
        original_srgba[1] * pose.tint_g,
        original_srgba[2] * pose.tint_b,
        original_srgba[3] * (pose.tint_a * pose.opacity),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clock(elapsed: f64, duration: f64, looping: bool) -> EffectClock {
        EffectClock {
            elapsed,
            duration,
            looping,
        }
    }
    #[test]
    fn f64_sprite_and_slide_are_nonterminal_when_projection_clocks_round_equal() {
        let source = clock(1.0, 1.000000000001, false);
        assert_eq!(source.elapsed as f32, source.duration as f32);
        let original_bits = (source.elapsed.to_bits(), source.duration.to_bits());
        let bounce = sample_sprite(
            &SpriteEffectKind::Bounce { height: 18.0 },
            &source,
            &TransformSnapshot::default(),
            false,
        );
        let slide = sample_visual(
            VisualTransition::SlideLeft,
            0.0,
            1.0,
            &source,
            &TransformSnapshot::default(),
            false,
        );
        assert!(bounce.translation[1] > 0.0);
        assert!(slide.transform.translation[0] > 0.0);
        assert_eq!(
            (source.elapsed.to_bits(), source.duration.to_bits()),
            original_bits
        );
    }
    #[test]
    fn sprite_terminal_and_reduced_motion_preserve_the_full_neutral_transform() {
        let neutral = TransformSnapshot {
            translation: [4.0, 5.0, 6.0],
            scale: [-1.25, 1.25, 1.0],
            rotation: [0.0, 0.0, 0.5, 0.8660254],
        };
        for animation in [
            SpriteEffectKind::Shake { intensity: 30.0 },
            SpriteEffectKind::Bounce { height: 18.0 },
            SpriteEffectKind::Pulse { scale: 1.2 },
        ] {
            assert_eq!(
                sample_sprite(&animation, &clock(1.0, 1.0, false), &neutral, false),
                neutral
            );
            assert_eq!(
                sample_sprite(&animation, &clock(0.25, 1.0, true), &neutral, true),
                neutral
            );
        }
    }
    #[test]
    fn loop_sampling_preserves_cumulative_clock_and_uses_its_fractional_phase() {
        let source = clock(20.25, 1.0, true);
        let start = TransformSnapshot::default();
        assert_eq!(
            sample_sprite(
                &SpriteEffectKind::Pulse { scale: 1.5 },
                &source,
                &start,
                false
            ),
            sample_sprite(
                &SpriteEffectKind::Pulse { scale: 1.5 },
                &clock(0.25, 1.0, true),
                &start,
                false
            )
        );
        assert_eq!(source.elapsed, 20.25);
    }
    #[test]
    fn fade_contribution_composes_once_over_the_animation_and_authored_alpha() {
        let original = TransformSnapshot {
            translation: [17.0, 23.0, 10.0],
            ..Default::default()
        };
        let source = clock(0.25, 1.0, false);
        let animated = sample_sprite(
            &SpriteEffectKind::Bounce { height: 18.0 },
            &source,
            &original,
            false,
        );
        let visual = sample_visual(
            VisualTransition::SlideLeft,
            0.0,
            1.0,
            &source,
            &animated,
            false,
        );
        assert_eq!(visual.transform.translation[0], 17.0 + 1440.0);
        assert_eq!(visual.transform.translation[1], animated.translation[1]);
        assert_eq!(visual.with_original_alpha(0.4), 0.1);
    }
    #[test]
    fn outgoing_fade_uses_frozen_alpha_and_own_clock_without_advancing_source() {
        let old = clock(0.25, 1.0, false);
        let new = clock(0.5, 2.0, false);
        let frozen = sample_visual(
            VisualTransition::Fade,
            0.0,
            1.0,
            &old,
            &TransformSnapshot::default(),
            false,
        );
        let outgoing = sample_visual(
            VisualTransition::Fade,
            1.0,
            0.0,
            &new,
            &frozen.transform,
            false,
        );
        assert_eq!(frozen.alpha * outgoing.alpha, 0.1875);
        assert_eq!(old.elapsed, 0.25);
    }
    #[test]
    fn retired_audio_multiplies_one_factor_over_the_frozen_absolute_gain() {
        let frozen = sample_audio(0.0, 0.32, &clock(0.5, 1.0, false));
        let factor = sample_audio(1.0, 0.0, &clock(0.25, 1.0, false));
        let sampled = retired_music_gain(frozen, &clock(0.25, 1.0, false));
        assert!((sampled - 0.12).abs() < 1e-7);
        assert_ne!(frozen * factor * 0.32, frozen * factor);
    }
    #[test]
    fn motion_geometry_keeps_ui_sign_and_noncentral_pivot_explicit() {
        let original = TransformSnapshot::default();
        let pose = Pose {
            x: 10.0,
            y: 20.0,
            rotation: 90.0,
            pivot_x: 1.0,
            pivot_y: 0.5,
            ..Default::default()
        };
        let sprite = compose_motion(
            &original,
            &pose,
            MotionGeometry {
                size: [100.0, 50.0],
                factor: [1.0, 1.0],
                ui: false,
            },
        );
        let interface = compose_motion(
            &original,
            &pose,
            MotionGeometry {
                size: [100.0, 50.0],
                factor: [1.0, 1.0],
                ui: true,
            },
        );
        assert!((sprite.translation[0] - 60.0).abs() < 1e-4);
        assert!((sprite.translation[1] - 30.0).abs() < 1e-4);
        assert!((interface.translation[0] - 60.0).abs() < 1e-4);
        assert!((interface.translation[1] + 30.0).abs() < 1e-4);
    }
    #[test]
    fn root_and_layer_color_contributions_are_separate_multipliers() {
        let root = Pose {
            tint_r: 0.5,
            tint_a: 0.8,
            opacity: 0.5,
            ..Default::default()
        };
        let layer = Pose {
            tint_g: 0.25,
            tint_a: 0.5,
            opacity: 0.25,
            ..Default::default()
        };
        let result = motion_color(motion_color([0.8, 0.6, 0.4, 0.4], &root), &layer);
        for (actual, expected) in result.into_iter().zip([0.4, 0.15, 0.4, 0.02]) {
            assert!((actual - expected).abs() < 1e-7);
        }
    }
    #[test]
    fn quaternion_with_allowed_norm_tolerance_keeps_the_original_pivot_scale() {
        let quaternion = [0.0, 0.0, 0.0, 1.0001f32];
        let norm = quaternion.iter().map(|value| value * value).sum::<f32>();
        assert!((norm - 1.0).abs() <= 0.001);
        let sampled = rotate(quaternion, [50.0, 0.0, 0.0]);
        assert!((sampled[0] - 50.01000213623047).abs() < 1e-5);
        assert!(sampled[0] > 50.0);
    }
    #[test]
    fn simple_fade_preserves_the_single_final_cast_with_original_alpha() {
        let sampled = sample_visual_at(
            VisualTransition::Fade,
            0.5078412890434265f32,
            0.587384819984436f32,
            &TransformSnapshot::default(),
            0.18466034385487662,
        );
        let original = 0.4656500816345215f32;
        assert_eq!(sampled.with_original_alpha(original).to_bits(), 0x3e7927d8);
        assert_ne!((original * sampled.alpha as f32).to_bits(), 0x3e7927d8);
    }
    #[test]
    fn motion_alpha_keeps_the_existing_multiplier_association() {
        let pose = Pose {
            tint_a: 0.5597723722457886,
            opacity: 0.9242106080055237,
            ..Default::default()
        };
        let original = 0.4523795545101166f32;
        let result = motion_color([1.0, 1.0, 1.0, original], &pose);
        assert_eq!(result[3].to_bits(), 0x3e6fa784);
        assert_eq!(
            ((original * pose.tint_a) * pose.opacity).to_bits(),
            0x3e6fa785
        );
    }
    #[test]
    fn reduced_visual_projection_uses_endpoint_without_mutating_clock_authority() {
        let clock = clock(0.25, 1.0, false);
        let bits = (clock.elapsed.to_bits(), clock.duration.to_bits());
        let original = TransformSnapshot {
            translation: [37.0, 23.0, 10.0],
            ..Default::default()
        };
        let visible = sample_visual(
            VisualTransition::SlideLeft,
            0.0,
            1.0,
            &clock,
            &original,
            true,
        );
        assert_eq!(visible.transform, original);
        assert_eq!(visible.alpha, 1.0);
        let outgoing = sample_visual(VisualTransition::Fade, 1.0, 0.0, &clock, &original, true);
        assert_eq!(outgoing.alpha, 0.0);
        assert_eq!(bits, (clock.elapsed.to_bits(), clock.duration.to_bits()));
    }
}
