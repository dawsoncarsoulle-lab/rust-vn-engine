//! Neutral descriptions for the sprite animations already implemented by Bevy.
use crate::scene_resources::{EffectClock, EffectParameters, SavedEffect, SpriteEffectKind};
use rvn_parser::{AnimationParam, AnimationValue};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpriteStyle {
    pub flip_x: bool,
    pub flip_y: bool,
    pub scale: f32,
    pub rotation: f32,
    pub tint: Option<String>,
}
impl Default for SpriteStyle {
    fn default() -> Self {
        Self {
            flip_x: false,
            flip_y: false,
            scale: 1.0,
            rotation: 0.0,
            tint: None,
        }
    }
}
impl SpriteStyle {
    pub fn validate(&self) -> Result<(), String> {
        if !self.scale.is_finite() || !self.rotation.is_finite() {
            return Err("Non-finite sprite style transform".into());
        }
        if self.tint.as_ref().is_some_and(|color| {
            let digits = color.strip_prefix('#').unwrap_or(color);
            !matches!(digits.len(), 3 | 4 | 6 | 8)
                || !digits.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err("Invalid hexadecimal sprite tint".into());
        }
        Ok(())
    }
}

pub fn prepare_sprite_animation(
    animation: &str,
    params: &[AnimationParam],
) -> Result<SavedEffect, String> {
    let number = |name: &str, fallback: f32| -> Result<f32, String> {
        let value = params
            .iter()
            .find(|param| param.name == name)
            .and_then(|param| match param.value {
                AnimationValue::Float(value) => Some(value),
                AnimationValue::Int(value) => Some(value as f32),
                _ => None,
            })
            .unwrap_or(fallback);
        if value.is_finite() {
            Ok(value)
        } else {
            Err(format!("Non-finite sprite animation parameter: {name}"))
        }
    };
    let (kind, duration) = match animation {
        "shake" => (
            SpriteEffectKind::Shake {
                intensity: number("intensity", 10.0)?.max(0.0),
            },
            0.35,
        ),
        "bounce" => (
            SpriteEffectKind::Bounce {
                height: number("height", 18.0)?.max(0.0),
            },
            0.45,
        ),
        "pulse" => (
            SpriteEffectKind::Pulse {
                scale: number("scale", 1.08)?.max(0.01),
            },
            0.60,
        ),
        other => return Err(format!("Unknown sprite animation: {other}")),
    };
    let looping = params
        .iter()
        .find(|param| param.name == "loop")
        .and_then(|param| {
            if let AnimationValue::Bool(value) = param.value {
                Some(value)
            } else {
                None
            }
        })
        .unwrap_or(false);
    let effect = SavedEffect {
        presentation: None,
        base_resolved: false,
        clock: EffectClock {
            duration: f64::from(number("duration", duration)?.max(0.01)),
            elapsed: 0.0,
            looping,
        },
        parameters: EffectParameters::SpriteAnimation {
            animation: kind,
            base: Default::default(),
        },
    };
    effect.validate()?;
    Ok(effect)
}
