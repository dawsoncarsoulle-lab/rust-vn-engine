// PRIVATE candidate shared geometry. No saved fields, clock, I/O or alias lookup.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BackgroundGeometry {
    pub pixels: Vec2,
    pub cover: Transform,
    pub motion_factor: Vec2,
}

/// `pixels` must belong to the exact selected, ready image handle. Sampling an
/// explicit effect base remains a later step; this does not overwrite that base.
pub(crate) fn resolve(viewport: Vec2, pixels: Vec2) -> Option<BackgroundGeometry> {
    if !viewport.is_finite() || !pixels.is_finite()
        || viewport.min_element() <= 0.0 || pixels.min_element() <= 0.0
    {
        return None;
    }
    let scale = (viewport.x / pixels.x).max(viewport.y / pixels.y);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    Some(BackgroundGeometry {
        pixels,
        cover: Transform {
            translation: Vec3::new(0.0, 0.0, -100.0),
            scale: Vec3::new(scale, scale, 1.0),
            ..default()
        },
        motion_factor: Vec2::new(viewport.x / 1920.0, viewport.y / 1080.0),
    })
}
