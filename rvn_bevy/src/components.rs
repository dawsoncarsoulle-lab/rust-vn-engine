use bevy::prelude::*;

#[derive(Component)]
pub struct VnBackground;

#[derive(Component)]
pub struct VnSprite {
    pub id: String,
}

#[derive(Component)]
pub struct DialogueBox;

#[derive(Component)]
pub struct CharacterNameText;

#[derive(Component)]
pub struct DialogueText;

#[allow(dead_code)]
#[derive(Component)]
pub struct ChoiceButton(pub usize);

#[derive(Component)]
pub struct ChoiceContainer;

#[derive(Component)]
pub struct MusicMarker;

#[derive(Component)]
pub struct SfxSource {
    pub file: String,
}

/// Animation d'opacité sur un sprite ou un fond (visuel).
#[derive(Component)]
pub struct FadeAnim {
    pub from: f32,
    pub to: f32,
    pub duration_secs: f32,
    pub elapsed_secs: f32,
    pub despawn_on_finish: bool,
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
