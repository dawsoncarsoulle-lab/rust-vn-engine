//! Language of authoring diagnostics, independent of game text and documents.
use std::sync::atomic::{AtomicU8, Ordering};

static LANGUAGE: AtomicU8 = AtomicU8::new(0);

/// Select before creating the editor UI. Does not translate document content.
pub fn set_diagnostic_english(english: bool) { LANGUAGE.store(if english { 2 } else { 1 }, Ordering::Relaxed); }
pub(crate) fn english() -> bool {
    match LANGUAGE.load(Ordering::Relaxed) {
        2 => true,
        1 => false,
        _ => std::env::var("RVN_UI_DIAGNOSTIC_LANGUAGE").is_ok_and(|value| value == "en"),
    }
}

// Both format branches are checked by Rust. Arguments (including user names)
// are interpolated verbatim, never translated or interpreted as format strings.
macro_rules! diagnostic {
    ($fr:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if crate::diagnostics::english() { format!($en $(, $arg)*) }
        else { format!($fr $(, $arg)*) }
    };
}
