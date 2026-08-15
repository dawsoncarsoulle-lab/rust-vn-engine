use bevy::prelude::*;

#[derive(Component)]
pub struct VnBackground;

#[derive(Component)]
pub struct VnCinematic;

#[derive(Component)]
pub struct VnSprite {
    pub id: String,
}

#[derive(Component, Clone, Copy)]
pub struct SpriteBaseTransform {
    pub translation: Vec3,
    pub scale: Vec3,
}

#[derive(Debug, Clone)]
pub enum AnimationKind {
    Shake { intensity: f32 },
    Bounce { height: f32 },
    Pulse { scale: f32 },
}

#[derive(Component, Debug, Clone)]
pub struct SpriteAnimation {
    pub kind: AnimationKind,
    pub duration_secs: f32,
    pub elapsed_secs: f32,
    pub looping: bool,
}

#[derive(Component)]
pub struct DialogueBox;

#[derive(Component)]
pub struct CharacterNameText;

#[derive(Component)]
pub struct DialogueText;

#[derive(Component)]
pub struct ChoiceButton(pub usize);

#[derive(Component)]
pub struct ChoiceContainer;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
pub struct MusicMarker;

#[derive(Component)]
pub struct SfxSource {
    pub file: String,
}

/// Kind of visual transition. Determines how the entity animates
/// in addition to (or instead of) a simple alpha fade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    Fade,
    Dissolve,
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    ZoomIn,
    ZoomOut,
    Wipe,
    Blur,
}

impl Default for TransitionKind {
    fn default() -> Self {
        TransitionKind::Fade
    }
}

/// Animation d'opacité et/ou de transform sur un sprite ou un fond (visuel).
#[derive(Component)]
pub struct FadeAnim {
    pub from: f32,
    pub to: f32,
    pub duration_secs: f32,
    pub elapsed_secs: f32,
    pub despawn_on_finish: bool,
    /// Visual transition style. When not Fade/Dissolve, the entity
    /// also animates its transform (slide/zoom) in addition to alpha.
    pub kind: TransitionKind,
}

/// Crossfade de volume sur une entité audio.
/// Ajouté à la piste entrante (fade-in) et sortante (fade-out).
#[derive(Component)]
pub struct AudioFade {
    /// Volume de départ (0.0–1.0).
    pub from_vol: f32,
    /// Volume d'arrivée (0.0–1.0).
    pub to_vol: f32,
    pub duration_secs: f32,
    pub elapsed_secs: f32,
    /// Si `true`, l'entité est despawnée quand le fade est terminé.
    /// Utilisé pour la piste sortante.
    pub despawn_on_done: bool,
}

#[derive(Component)]
pub struct ImagemapBackground;

#[derive(Component)]
pub struct ImagemapHover;
