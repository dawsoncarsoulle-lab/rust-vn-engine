//! MIT authoring/runtime bridge; the separately distributed FFmpeg libraries
//! retain their own LGPL license and corresponding-source requirements.
#[cfg(all(feature = "video", not(target_arch = "wasm32")))]
pub mod decoder;
#[cfg(all(feature = "video", not(target_arch = "wasm32")))]
pub mod transport;
