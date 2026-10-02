//! Saveable media state, independent from a desktop decoder or an HTML element.
pub use rvn_ui::video::VideoClip;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Playback {
    Playing,
    Paused,
    Ended,
    Failed,
    Blocked,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub clip: VideoClip,
    pub position: f64,
    pub duration: Option<f64>,
    pub playback: Playback,
    pub message: Option<String>,
}
impl Track {
    pub fn validate(&self) -> Result<(), String> {
        self.clip.validate()?;
        if !self.position.is_finite()
            || !(0.0..=86_400.0).contains(&self.position)
            || self.duration.is_some_and(|duration| {
                !duration.is_finite()
                    || !(0.001..=86_400.0).contains(&duration)
                    || self.position > duration + 0.001
            })
        {
            return Err("Invalid saved video position or duration".into());
        }
        if self
            .message
            .as_ref()
            .is_some_and(|message| message.is_empty() || message.len() > 4096)
            || matches!(self.playback, Playback::Failed | Playback::Blocked)
                != self.message.is_some()
        {
            return Err("Video failure or autoplay refusal needs a bounded explanation".into());
        }
        if self.playback == Playback::Ended && self.clip.looping {
            return Err("A looping video cannot be saved as ended".into());
        }
        Ok(())
    }
    pub fn finished(&self) -> bool {
        matches!(self.playback, Playback::Ended | Playback::Failed)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoState {
    pub tracks: BTreeMap<String, Track>,
    pub waiting: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct VideoView {
    pub id: String,
    pub epoch: u64,
    pub track: Track,
    pub subtitle: Option<String>,
}
#[derive(Clone, Debug)]
pub enum Feedback {
    Position { seconds: f64, duration: f64 },
    End,
    Error(String),
    AutoplayBlocked(String),
}
impl VideoState {
    pub fn validate(&self) -> Result<(), String> {
        if self.tracks.len() > 8 {
            return Err("Video limit: eight simultaneous players".into());
        }
        for (id, track) in &self.tracks {
            if !rvn_ui::composition::valid_name(id) {
                return Err("Invalid video player identity".into());
            }
            track.validate()?;
        }
        if let Some(waiting) = &self.waiting {
            let track = self
                .tracks
                .get(waiting)
                .ok_or("Saved video wait references a missing player")?;
            if track.clip.looping || track.finished() {
                return Err("Cannot wait for an endless or already finished video".into());
            }
        }
        Ok(())
    }
    pub fn play(&mut self, id: String, clip: VideoClip) -> Result<(), String> {
        clip.validate()?;
        if !rvn_ui::composition::valid_name(&id) {
            return Err("Video identity needs 1–128 characters without / or :".into());
        }
        if self.tracks.contains_key(&id) && self.waiting.as_ref() == Some(&id) {
            // Replacing a player must not inherit another clip's wait.
            self.waiting = None;
        }
        if !self.tracks.contains_key(&id) && self.tracks.len() >= 8 {
            return Err("Video limit: eight simultaneous players".into());
        }
        self.tracks.insert(
            id,
            Track {
                clip,
                position: 0.0,
                duration: None,
                playback: Playback::Playing,
                message: None,
            },
        );
        Ok(())
    }
    pub fn stop(&mut self, id: &str) {
        self.tracks.remove(id);
        if self.waiting.as_deref() == Some(id) {
            self.waiting = None;
        }
    }
    pub fn wait(&mut self, id: &str) -> Result<(), String> {
        let track = self
            .tracks
            .get(id)
            .ok_or_else(|| format!("Unknown video player '{id}'"))?;
        if track.clip.looping {
            return Err("Cannot wait for a looping video; stop it explicitly".into());
        }
        if !track.finished() {
            self.waiting = Some(id.into());
        }
        Ok(())
    }
    fn track_mut(&mut self, id: &str) -> Result<&mut Track, String> {
        self.tracks
            .get_mut(id)
            .ok_or_else(|| format!("Unknown video player '{id}'"))
    }
    pub fn pause(&mut self, id: &str) -> Result<(), String> {
        let track = self.track_mut(id)?;
        if track.playback == Playback::Playing {
            track.playback = Playback::Paused;
        }
        Ok(())
    }
    pub fn resume(&mut self, id: &str) -> Result<(), String> {
        let track = self.track_mut(id)?;
        if matches!(track.playback, Playback::Paused | Playback::Blocked) {
            track.playback = Playback::Playing;
            track.message = None;
        } else if track.finished() {
            return Err("A finished video needs video.play or video.seek before resume".into());
        }
        Ok(())
    }
    pub fn seek(&mut self, id: &str, seconds: f64) -> Result<(), String> {
        let track = self.track_mut(id)?;
        if !seconds.is_finite()
            || !(0.0..=86_400.0).contains(&seconds)
            || track.duration.is_some_and(|duration| seconds > duration)
        {
            return Err("Video seek position is outside the clip".into());
        }
        track.position = seconds;
        if track.finished() {
            track.playback = Playback::Paused;
            track.message = None;
        }
        Ok(())
    }
    pub fn volume(&mut self, id: &str, volume: f64) -> Result<(), String> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return Err("Video volume must be between 0 and 1".into());
        }
        self.track_mut(id)?.clip.volume = volume as f32;
        Ok(())
    }
    /// Returns an event handler exactly once per terminal transition. The host
    /// separately checks the playback epoch before accepting decoder feedback.
    pub fn feedback(&mut self, id: &str, feedback: Feedback) -> Result<Option<String>, String> {
        let track = self.track_mut(id)?;
        if track.finished() {
            return Ok(None);
        }
        let handler = match feedback {
            Feedback::Position { seconds, duration } => {
                if !seconds.is_finite()
                    || !duration.is_finite()
                    || !(0.001..=86_400.0).contains(&duration)
                    || !(0.0..=duration + 0.001).contains(&seconds)
                {
                    return Err("Video renderer reported an invalid position".into());
                }
                track.position = seconds.min(duration);
                track.duration = Some(duration);
                None
            }
            Feedback::End => {
                if track.clip.looping {
                    return Err("Looping video unexpectedly ended".into());
                }
                if let Some(duration) = track.duration {
                    track.position = duration;
                }
                track.playback = Playback::Ended;
                track.message = None;
                track.clip.on_end.clone()
            }
            Feedback::Error(message) => {
                if message.is_empty() || message.len() > 4096 {
                    return Err("Invalid video renderer error message".into());
                }
                track.playback = Playback::Failed;
                track.message = Some(message);
                track.clip.on_error.clone()
            }
            Feedback::AutoplayBlocked(message) => {
                if message.is_empty() || message.len() > 4096 {
                    return Err("Invalid autoplay refusal message".into());
                }
                track.playback = Playback::Blocked;
                track.message = Some(message);
                None
            }
        };
        if self.tracks[id].finished() && self.waiting.as_deref() == Some(id) {
            self.waiting = None;
        }
        Ok(handler)
    }
    pub fn autoplay_blocked(&mut self, id: &str, message: String) -> Result<(), String> {
        self.feedback(id, Feedback::AutoplayBlocked(message))?;
        Ok(())
    }
}

pub fn construct(args: &[rvn_parser::Value]) -> Result<rvn_parser::Value, crate::eval::EvalError> {
    use rvn_parser::Value;
    let invalid = |message: &str| crate::eval::EvalError::InvalidFunction(message.into());
    let [Value::Str(source), Value::Dict(options)] = args else {
        return Err(invalid(
            "video_clip expects a resource path and a properties dictionary",
        ));
    };
    if options.contains_key("source") {
        return Err(invalid(
            "Video source belongs in video_clip's first argument",
        ));
    }
    let mut description = options.clone();
    description.insert("source".into(), Value::Str(source.clone()));
    let value = Value::Dict(description);
    VideoClip::parse(crate::ui::value_to_json(&value)?).map_err(|message| invalid(&message))?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clip() -> VideoClip {
        VideoClip::parse(
            serde_json::json!({"source":"intro.webm","skippable":true,"on_end":"finished"}),
        )
        .unwrap()
    }
    #[test]
    fn position_pause_seek_events_and_wait_survive_serialization() {
        let mut state = VideoState::default();
        state.play("intro".into(), clip()).unwrap();
        state.wait("intro").unwrap();
        state
            .feedback(
                "intro",
                Feedback::Position {
                    seconds: 0.5,
                    duration: 2.0,
                },
            )
            .unwrap();
        state.pause("intro").unwrap();
        let decoded: VideoState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, state);
        state.resume("intro").unwrap();
        state.seek("intro", 1.0).unwrap();
        assert_eq!(
            state.feedback("intro", Feedback::End).unwrap(),
            Some("finished".into())
        );
        assert!(state.waiting.is_none());
        assert_eq!(state.feedback("intro", Feedback::End).unwrap(), None);
        state.seek("intro", 0.0).unwrap();
        assert_eq!(state.tracks["intro"].playback, Playback::Paused);
        state.resume("intro").unwrap();
        assert!(state.seek("intro", 3.0).is_err());
        assert!(state.volume("intro", f64::NAN).is_err());
    }
    #[test]
    fn loops_errors_and_autoplay_refusals_do_not_silently_complete() {
        let mut state = VideoState::default();
        let mut clip = clip();
        clip.looping = true;
        state.play("intro".into(), clip).unwrap();
        assert!(state.wait("intro").is_err());
        assert!(state.feedback("intro", Feedback::End).is_err());
        state
            .autoplay_blocked("intro", "Click to allow video playback".into())
            .unwrap();
        state.validate().unwrap();
        assert_eq!(state.tracks["intro"].playback, Playback::Blocked);
        state.resume("intro").unwrap();
        state.validate().unwrap();
        state
            .feedback("intro", Feedback::Error("Missing resource".into()))
            .unwrap();
        state.validate().unwrap();
        state.stop("intro");
        assert!(state.tracks.is_empty());
    }
}
