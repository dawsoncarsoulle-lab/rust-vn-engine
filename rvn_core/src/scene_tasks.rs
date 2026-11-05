//! Saved task descriptions and transient callback capabilities.
use crate::scene::ActivationId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskOwner {
    Game,
    Scene { activation: ActivationId },
    Host { role: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskKey {
    pub owner: TaskOwner,
    pub local_id: String,
}

/// A token is never serialized: a load must revoke every earlier callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskToken {
    pub runtime_epoch: u64,
    pub key: TaskKey,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimerTask {
    pub duration: f64,
    pub elapsed: f64,
    pub action: String,
    pub revision: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneTasks {
    pub next_revision: u64,
    pub timer: Option<TimerTask>,
    /// These snapshots are filled on suspension, and moved back on return.
    /// They contain no decoder, entity, GPU texture, pointer capture or window.
    pub suspended_motions: crate::motion::MotionState,
    pub suspended_videos: crate::video::VideoState,
    pub suspended_ui: crate::ui::UiState,
    #[serde(default)]
    pub resources_parked: bool,
}

impl SceneTasks {
    pub fn renew_callbacks(&mut self) -> Result<(), crate::eval::EvalError> {
        let revision = self
            .next_revision
            .checked_add(1)
            .ok_or(crate::eval::EvalError::NumericOverflow)?;
        self.next_revision = revision;
        if let Some(timer) = &mut self.timer {
            timer.revision = revision;
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Some(timer) = &self.timer {
            if !timer.duration.is_finite()
                || timer.duration < 0.0
                || !timer.elapsed.is_finite()
                || timer.elapsed < 0.0
                || timer.elapsed > timer.duration
                || timer.revision == 0
                || timer.revision > self.next_revision
                || timer.action.len() > 4096
                || !valid_timer_action(&timer.action)
            {
                return Err("Invalid saved scene timer".into());
            }
        }
        if self
            .suspended_ui
            .screens
            .iter()
            .any(|screen| screen.host_role.is_some() || screen.host_root)
        {
            return Err("Host interfaces cannot be parked in a story visit".into());
        }
        crate::scene_resources::validate_snapshot_clocks(
            &self.suspended_motions,
            &self.suspended_videos,
        )?;
        crate::scene_resources::validate_ui_state(
            &self.suspended_ui,
            &mut crate::scene::NarrativeBudget::default(),
        )?;
        Ok(())
    }

    pub fn set_timer(&mut self, duration: f64, action: String) -> Result<(), String> {
        if !duration.is_finite()
            || duration < 0.0
            || action.len() > 4096
            || !valid_timer_action(&action)
        {
            return Err("A timer needs a finite positive duration and jump/call target".into());
        }
        let revision = self
            .next_revision
            .checked_add(1)
            .ok_or("Task revision exhausted")?;
        self.next_revision = revision;
        self.timer = Some(TimerTask {
            duration,
            elapsed: 0.0,
            action,
            revision,
        });
        Ok(())
    }

    pub fn advance_timer(&mut self, delta: f64) -> Result<Option<TimerTask>, String> {
        if !delta.is_finite() || delta < 0.0 {
            return Err("A task delta must be finite and nonnegative".into());
        }
        let Some(timer) = &mut self.timer else {
            return Ok(None);
        };
        timer.elapsed = (timer.elapsed + delta).min(timer.duration);
        // A due action can be suspended by a game-owned callback earlier in
        // the same frame. Keep it due until its own owner can consume it.
        if timer.elapsed >= timer.duration {
            Ok(Some(timer.clone()))
        } else {
            Ok(None)
        }
    }
}

pub fn valid_timer_action(action: &str) -> bool {
    let parts: Vec<_> = action.split_whitespace().collect();
    matches!(parts.as_slice(), ["jump" | "call", target] if rvn_parser::is_binding_name(target))
}
