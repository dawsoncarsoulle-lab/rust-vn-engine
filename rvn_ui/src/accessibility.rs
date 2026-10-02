//! Portable accessibility policy. Platform voices and speech handles are never
//! serialized; player overrides take precedence over the author's settings.
use serde::{Deserialize, Serialize};

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpeechRequest {
    Speak(String),
    Stop,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccessibilitySettings {
    pub self_voicing: bool,
    #[serde(default = "one")]
    pub speech_rate: f32,
    #[serde(default = "one")]
    pub speech_volume: f32,
    pub language: Option<String>,
    #[serde(default = "one")]
    pub text_scale: f32,
    pub high_contrast: bool,
    pub reduced_motion: bool,
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            self_voicing: false,
            speech_rate: 1.0,
            speech_volume: 1.0,
            language: None,
            text_scale: 1.0,
            high_contrast: false,
            reduced_motion: false,
        }
    }
}

impl AccessibilitySettings {
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        let settings: Self = serde_json::from_value(value)
            .map_err(|e| format!("Invalid accessibility settings: {e}"))?;
        settings.validate()?;
        Ok(settings)
    }
    pub fn validate(&self) -> Result<(), String> {
        for (name, value, min, max) in [
            ("speech_rate", self.speech_rate, 0.5, 2.0),
            ("speech_volume", self.speech_volume, 0.0, 1.0),
            ("text_scale", self.text_scale, 0.75, 2.5),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!(
                    "Accessibility {name} must be between {min} and {max}"
                ));
            }
        }
        if self.language.as_ref().is_some_and(|lang| {
            lang.is_empty()
                || lang.len() > 64
                || lang
                    .split('-')
                    .any(|part| part.is_empty() || !part.chars().all(|c| c.is_ascii_alphanumeric()))
        }) {
            return Err(
                "Speech language must be a language tag, for example en-US or fr-FR".into(),
            );
        }
        Ok(())
    }
}

pub fn validate_speech(text: &str) -> Result<(), String> {
    if text.trim().is_empty() || text.len() > 16_384 || text.contains('\0') {
        Err("Speech needs non-empty text, at most 16384 bytes, without NUL characters".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_partial_policy_are_portable() {
        assert_eq!(
            AccessibilitySettings::parse(serde_json::json!({})).unwrap(),
            AccessibilitySettings::default()
        );
        let settings = AccessibilitySettings::parse(
            serde_json::json!({"self_voicing":true,"language":"fr-FR","text_scale":1.5}),
        )
        .unwrap();
        assert_eq!(settings.speech_rate, 1.0);
        assert_eq!(
            serde_json::from_str::<AccessibilitySettings>(
                &serde_json::to_string(&settings).unwrap()
            )
            .unwrap(),
            settings
        );
    }
    #[test]
    fn typos_unbounded_values_and_invalid_languages_are_errors() {
        for value in [
            serde_json::json!({"text_size":2}),
            serde_json::json!({"text_scale":100}),
            serde_json::json!({"speech_rate":0}),
            serde_json::json!({"speech_volume":2}),
            serde_json::json!({"language":"en; command"}),
            serde_json::json!({"language":"en--US"}),
        ] {
            assert!(AccessibilitySettings::parse(value).is_err());
        }
        for text in ["", "   ", "bad\0text"] {
            assert!(validate_speech(text).is_err());
        }
        assert!(validate_speech(&"a".repeat(16_385)).is_err());
        assert!(validate_speech("Bonjour, bienvenue !").is_ok());
    }
}
