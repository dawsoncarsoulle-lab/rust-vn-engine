//! Portable observed dialogue progress. No engine epochs, ECS entities or
//! player preferences enter the save; delivery authority stays transient.
use serde::{Deserialize, Serialize};

pub const MAX_REVEAL_TEXT_BYTES: usize = 1_000_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevealCheckpoint {
    pub rendered_text: String,
    pub total_chars: u32,
    pub visible_chars: u32,
    pub typing: bool,
    pub pause_remaining: f32,
    /// Existing clock backlog is not necessarily a fraction smaller than one.
    pub char_progress: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayRevealSource {
    pub engine_instance: u64,
    pub runtime_epoch: u64,
    pub display_generation: u64,
    pub position: crate::scene::ResumePosition,
    pub owner: Option<crate::scene::ActivationId>,
}

/// Match the existing dialogue renderer's literal fallback for invalid tags.
/// Save validation and both platform presenters count the same Unicode scalars.
pub fn rendered_rich_text(text: &str) -> crate::RichText {
    crate::parse_text_tags(text).unwrap_or_else(|_| {
        let escaped = text.replace('{', "{{").replace('}', "}}");
        crate::parse_text_tags(&escaped).unwrap_or_else(|_| crate::RichText {
            segments: vec![crate::RichTextSegment {
                text: text.to_owned(), color: None, speed: None, shake: false,
                pause_after: None, bold: false, italic: false, underline: false,
                size: None, alpha: None,
            }],
        })
    })
}

impl RevealCheckpoint {
    pub fn observed(text: &str, visible: usize, typing: bool, pause: f32, progress: f32) -> Result<Self, String> {
        if text.len() > MAX_REVEAL_TEXT_BYTES {
            return Err("Dialogue reveal exceeds the portable text budget".into());
        }
        let total = rendered_rich_text(text).plain_text().chars().count();
        let checkpoint = Self {
            rendered_text: text.to_owned(),
            total_chars: u32::try_from(total).map_err(|_| "Dialogue character count overflow")?,
            visible_chars: u32::try_from(visible).map_err(|_| "Dialogue cursor overflow")?,
            typing, pause_remaining: pause, char_progress: progress,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.rendered_text.len() > MAX_REVEAL_TEXT_BYTES
            || !self.pause_remaining.is_finite() || self.pause_remaining < 0.0
            || !self.char_progress.is_finite() || self.char_progress < 0.0 {
            return Err("Invalid portable dialogue reveal metrics".into());
        }
        let count = rendered_rich_text(&self.rendered_text).plain_text().chars().count();
        if usize::try_from(self.total_chars).ok() != Some(count) || self.visible_chars > self.total_chars
            || (!self.typing && self.visible_chars != self.total_chars) {
            return Err("Dialogue reveal cursor disagrees with its Unicode text".into());
        }
        Ok(())
    }

    /// VN language adaptation: keep the absolute Unicode cursor, bounded by
    /// the new text, and keep the clock backlog/pause. A completed line stays
    /// completed. Identical text preserves every saved metric exactly.
    pub fn for_text(&self, text: &str) -> Result<Self, String> {
        self.validate()?;
        if text == self.rendered_text { return Ok(self.clone()); }
        let mut next = Self::observed(text, 0, true, self.pause_remaining, self.char_progress)?;
        if !self.typing || self.visible_chars == self.total_chars {
            next.visible_chars = next.total_chars;
            next.typing = false;
            next.pause_remaining = 0.0;
        } else {
            next.visible_chars = self.visible_chars.min(next.total_chars);
            if next.visible_chars == next.total_chars {
                next.typing = false;
                next.pause_remaining = 0.0;
            }
        }
        Ok(next)
    }
}

pub(crate) fn validate_state(state: &crate::GameState) -> Result<(), String> {
    let Some(display) = state.scene_execution.as_ref().and_then(|execution| execution.display.as_ref()) else { return Ok(()); };
    if let Some(reveal) = &display.reveal {
        if !matches!(display.display, crate::HistoryDisplay::Dialogue { .. }) {
            return Err("Only a dialogue can carry a reveal checkpoint".into());
        }
        reveal.validate()?;
    }
    Ok(())
}
