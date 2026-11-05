//! Typewriter settings use the existing scene preflight/publication and the
//! historical infallible legacy renderer callback. The gathered candidate is
//! retained intact, including its actual consumer frame and random state.
use super::*;

impl<R: Renderer> Engine<R> {
    pub(super) fn typewriter_enabled_value(&self, value: Value) -> Result<bool, RuntimeError> {
        match value {
            Value::Bool(enabled) => Ok(enabled),
            _ => Err(self.eval_err(EvalError::InvalidFunction(
                "Typewriter enabled requires a Boolean value".into()), "Typewriter enabled")),
        }
    }
    pub(super) fn typewriter_speed_value(&self, value: Value) -> Result<u32, RuntimeError> {
        if let Value::Int(value) = value {
            if let Ok(speed) = u32::try_from(value) { return Ok(speed); }
        }
        Err(self.eval_err(EvalError::InvalidFunction(
            "Typewriter speed requires an integer in 0..=4294967295".into()), "Typewriter speed"))
    }
    pub(super) fn commit_typewriter_speed(
        &mut self,
        mut next: GameState,
        chars_per_sec: u32,
    ) -> Result<(), RuntimeError> {
        next.typewriter.chars_per_sec = chars_per_sec;
        next.typewriter.enabled = chars_per_sec > 0;
        next.pc += 1;
        if self.narrative.is_some() { return self.commit_scene_state(next); }
        self.state = next;
        self.renderer.set_typewriter_config(self.state.typewriter.effective_speed());
        Ok(())
    }
    pub(super) fn commit_typewriter_setting(
        &mut self,
        mut next: GameState,
        enabled: bool,
    ) -> Result<(), RuntimeError> {
        next.typewriter.enabled = enabled;
        next.pc += 1;
        if self.narrative.is_some() {
            return self.commit_scene_state(next);
        }
        self.state = next;
        self.renderer
            .set_typewriter_config(self.state.typewriter.effective_speed());
        Ok(())
    }
}
