//! Portable video descriptions. Decoder handles and browser objects never enter
//! an RVN value or a save file.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

fn one() -> f32 {
    1.0
}
fn full_screen() -> [f32; 4] {
    [0.0, 0.0, 1280.0, 720.0]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subtitle {
    pub start: f64,
    pub end: f64,
    /// A locale key, falling back to this text if no translation exists.
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoClip {
    pub source: String,
    #[serde(default)]
    pub poster: Option<String>,
    #[serde(default)]
    pub fallback: Option<String>,
    /// A separate, synchronized grayscale VP8 stream supplies transparency.
    #[serde(default)]
    pub mask: Option<String>,
    #[serde(default = "full_screen")]
    pub rect: [f32; 4],
    #[serde(default)]
    pub layer: i32,
    /// None means the scene; otherwise ui:<screen>/<component>.
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub cinematic: bool,
    #[serde(default)]
    pub skippable: bool,
    #[serde(default)]
    pub looping: bool,
    #[serde(default = "one")]
    pub volume: f32,
    #[serde(default)]
    pub keep_last_frame: bool,
    #[serde(default)]
    pub subtitles: Vec<Subtitle>,
    #[serde(default)]
    pub on_end: Option<String>,
    #[serde(default)]
    pub on_error: Option<String>,
}

impl VideoClip {
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        let clip: Self =
            serde_json::from_value(value).map_err(|e| format!("Invalid video description: {e}"))?;
        clip.validate()?;
        Ok(clip)
    }
    pub fn validate(&self) -> Result<(), String> {
        let video_path = |path: &str| {
            crate::programmable::safe_asset_path(path)
                && path.to_ascii_lowercase().ends_with(".webm")
        };
        if !video_path(&self.source) || self.mask.as_ref().is_some_and(|path| !video_path(path)) {
            return Err("Video and mask need project-relative .webm resources (VP8/Vorbis)".into());
        }
        if [&self.poster, &self.fallback]
            .into_iter()
            .flatten()
            .any(|path| !crate::programmable::safe_asset_path(path))
        {
            return Err("Video poster and fallback must be project-relative images".into());
        }
        let [x, y, w, h] = self.rect;
        if ![x, y, w, h].iter().all(|v| v.is_finite())
            || x.abs() > 8192.0
            || y.abs() > 8192.0
            || !(0.001..=8192.0).contains(&w)
            || !(0.001..=8192.0).contains(&h)
        {
            return Err(
                "Video rectangle must be finite with positive dimensions, at most 8192 pixels"
                    .into(),
            );
        }
        if !(-1024..=1024).contains(&self.layer) {
            return Err("Video layer must be between -1024 and 1024".into());
        }
        if !self.volume.is_finite() || !(0.0..=1.0).contains(&self.volume) {
            return Err("Video volume must be between 0 and 1".into());
        }
        if let Some(target) = &self.target {
            if !target
                .strip_prefix("ui:")
                .and_then(|value| value.split_once('/'))
                .is_some_and(|(screen, element)| {
                    crate::composition::valid_name(screen)
                        && crate::composition::valid_name(element)
                })
            {
                return Err("Embedded video target must be ui:<screen>/<component>".into());
            }
        }
        if self.cinematic && (self.looping || self.target.is_some()) {
            return Err(
                "A cinematic must be a finite scene video, not a looping interface video".into(),
            );
        }
        if self.subtitles.len() > 1024 {
            return Err("Video subtitle limit: 1024 cues".into());
        }
        let mut last_end = 0.0;
        for cue in &self.subtitles {
            if !cue.start.is_finite()
                || !cue.end.is_finite()
                || cue.start < last_end
                || cue.end <= cue.start
                || cue.end > 86_400.0
                || cue.text.is_empty()
                || cue.text.len() > 4096
            {
                return Err(
                    "Subtitles need ordered, non-overlapping, finite times and 1–4096 text bytes"
                        .into(),
                );
            }
            last_end = cue.end;
        }
        for handler in [&self.on_end, &self.on_error].into_iter().flatten() {
            if handler.is_empty()
                || handler.len() > 128
                || !handler
                    .chars()
                    .enumerate()
                    .all(|(i, c)| c == '_' || c.is_alphabetic() || (i > 0 && c.is_ascii_digit()))
            {
                return Err("Video event handler must be a function identifier".into());
            }
        }
        Ok(())
    }
    pub fn resources(&self) -> BTreeSet<String> {
        std::iter::once(&self.source)
            .chain(
                [&self.mask, &self.poster, &self.fallback]
                    .into_iter()
                    .flatten(),
            )
            .cloned()
            .collect()
    }
    pub fn subtitle(&self, seconds: f64) -> Option<&Subtitle> {
        self.subtitles
            .iter()
            .find(|cue| cue.start <= seconds && seconds < cue.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_defaults_subtitle_boundaries_and_resources() {
        let clip=VideoClip::parse(serde_json::json!({"source":"movies/intro.webm","poster":"images/poster.png","mask":"movies/alpha.webm","subtitles":[{"start":0,"end":1,"text":"intro.line"}]})).unwrap();
        assert_eq!(clip.volume, 1.0);
        assert_eq!(clip.rect, [0.0, 0.0, 1280.0, 720.0]);
        assert_eq!(clip.resources().len(), 3);
        assert!(clip.subtitle(0.0).is_some());
        assert!(clip.subtitle(1.0).is_none());
    }
    #[test]
    fn unsafe_paths_typos_invalid_times_and_conflicting_modes_are_errors() {
        for value in [
            serde_json::json!({"source":"../private.webm"}),
            serde_json::json!({"source":"https://example.org/a.webm"}),
            serde_json::json!({"source":"intro.mp4"}),
            serde_json::json!({"source":"intro.webm","volum":0.3}),
            serde_json::json!({"source":"intro.webm","volume":2}),
            serde_json::json!({"source":"intro.webm","rect":[0,0,0,20]}),
            serde_json::json!({"source":"intro.webm","target":"ui:menu/../x"}),
            serde_json::json!({"source":"intro.webm","cinematic":true,"looping":true}),
            serde_json::json!({"source":"intro.webm","on_end":"bad()"}),
            serde_json::json!({"source":"intro.webm","subtitles":[{"start":2,"end":1,"text":"No"}]}),
            serde_json::json!({"source":"intro.webm","subtitles":[{"start":0,"end":2,"text":"A"},{"start":1,"end":3,"text":"B"}]}),
        ] {
            assert!(VideoClip::parse(value).is_err());
        }
    }
}
