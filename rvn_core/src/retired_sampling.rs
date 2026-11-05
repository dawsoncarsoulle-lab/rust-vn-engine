//! PRIVATE, NOT_RUN. Pure sampling of historical descriptions.
//! Geometry is supplied from the frozen logical model, never a Current alias.
//! The temporary cache is not a second saved model or a second clock.
use crate::effect_sampling::{self, MotionGeometry};
use crate::retired_presentations::{FrozenVisualSample, RetiredModel, RetiredPresentation};
use crate::scene_resources::{EffectClock, EffectParameters, SavedEffect, TransformSnapshot};
use crate::sprite_effects::SpriteStyle;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct NeutralGeometry {
    /// Unstyled logical placement. SpriteStyle is applied from the payload.
    /// A layer is local to the root; its size/factor are adapter-derived.
    pub transform: TransformSnapshot,
    pub motion: MotionGeometry,
}
#[derive(Clone, Debug)]
pub struct LayerGeometry {
    /// Required exactly when this layer has a historical task sample.
    /// An unsampled selected layer needs no invented registry identity.
    pub source: Option<crate::scene_resources::TargetIdentity>,
    pub neutral: NeutralGeometry,
}
#[derive(Clone, Debug)]
pub struct HistoricalGeometry {
    pub source: crate::scene_resources::TargetIdentity,
    pub root: NeutralGeometry,
    pub layers: BTreeMap<String, LayerGeometry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RetiredLeaf {
    pub layer: Option<String>,
    pub path: String,
    /// Local under the returned root. Simple images use identity here.
    pub pose: TransformSnapshot,
    pub color: [f32; 4],
}
#[derive(Clone, Debug, PartialEq)]
pub enum RetiredFrame {
    Visual {
        root: TransformSnapshot,
        leaves: Vec<RetiredLeaf>,
    },
    Music {
        file: String,
        gain: f32,
    },
}
#[derive(Clone, Debug)]
pub struct FrozenAppearance {
    // Match a cached sample to the same immutable payload. The exact task/token
    // check remains at the caller boundary; no alias-only cache is sufficient.
    source_bytes: Vec<u8>,
    frame: RetiredFrame,
    layered: bool,
    reduced: bool,
}

fn validate_transform(base: &TransformSnapshot) -> Result<(), String> {
    SavedEffect {
        presentation: None,
        base_resolved: true,
        clock: EffectClock {
            duration: 1.0,
            elapsed: 0.0,
            looping: false,
        },
        parameters: EffectParameters::VisualFade {
            from: 0.0,
            to: 1.0,
            despawn_on_finish: false,
            transition: crate::scene_resources::VisualTransition::Fade,
            base: base.clone(),
            layered: false,
        },
    }
    .validate()
}
fn validate_geometry(geometry: &NeutralGeometry) -> Result<(), String> {
    validate_transform(&geometry.transform)?;
    if geometry.motion.ui
        || geometry
            .motion
            .size
            .iter()
            .chain(&geometry.motion.factor)
            .any(|value| !value.is_finite() || *value <= 0.0 || *value > 1.0e6)
    {
        return Err("Retired world geometry must be known, positive, finite and bounded".into());
    }
    Ok(())
}
fn styled_base(
    geometry: &NeutralGeometry,
    style: &SpriteStyle,
) -> Result<TransformSnapshot, String> {
    style.validate()?;
    let mut base = geometry.transform.clone();
    base.scale[0] = if style.flip_x {
        -style.scale
    } else {
        style.scale
    };
    base.scale[1] = if style.flip_y {
        -style.scale
    } else {
        style.scale
    };
    let half = style.rotation.to_radians() * 0.5;
    base.rotation = [0.0, 0.0, half.sin(), half.cos()];
    // A finite authored style is still valid at authoring time. Its resolved
    // presentation can exceed the existing finite transform budget; refuse it
    // here, without silently clamping or narrowing SpriteStyle itself.
    validate_transform(&base)?;
    Ok(base)
}
fn tint(style: &SpriteStyle) -> Result<[f32; 4], String> {
    style.validate()?;
    let Some(raw) = &style.tint else {
        return Ok([1.0; 4]);
    };
    let raw = raw.strip_prefix('#').unwrap_or(raw);
    let expanded;
    let digits = if matches!(raw.len(), 3 | 4) {
        expanded = raw.chars().flat_map(|c| [c, c]).collect::<String>();
        expanded.as_str()
    } else {
        raw
    };
    let mut color = [1.0; 4];
    for (i, component) in color.iter_mut().enumerate().take(digits.len() / 2) {
        *component = f32::from(
            u8::from_str_radix(&digits[i * 2..i * 2 + 2], 16).map_err(|_| "Invalid frozen tint")?,
        ) / 255.0;
    }
    Ok(color)
}
fn source_pose(
    sample: &FrozenVisualSample,
    neutral: &TransformSnapshot,
    reduced: bool,
) -> (TransformSnapshot, f64) {
    let mut root = neutral.clone();
    if let Some(sprite) = &sample.sprite {
        root = effect_sampling::sample_sprite(&sprite.animation, &sprite.clock, &root, reduced);
    }
    let mut alpha = 1.0;
    if let Some(fade) = &sample.fade {
        let visual = effect_sampling::sample_visual(
            fade.transition,
            fade.from,
            fade.to,
            &fade.clock,
            &root,
            reduced,
        );
        root = visual.transform;
        alpha = visual.alpha;
    }
    (root, alpha)
}
fn sprite_path(id: &str, state: &crate::types::SpriteState) -> String {
    match &state.emotion {
        Some(path) if path.contains('/') => path.strip_prefix("assets/").unwrap_or(path).into(),
        Some(emotion) => format!("sprites/{id}/{emotion}.png"),
        None => format!("sprites/{id}/default.png"),
    }
}
fn validate_frame(frame: &RetiredFrame) -> Result<(), String> {
    match frame {
        RetiredFrame::Visual { root, leaves } => {
            validate_transform(root)?;
            if leaves.len() > 128 {
                return Err("Retired projection exceeds 128 leaves".into());
            }
            for leaf in leaves {
                validate_transform(&leaf.pose)?;
                if !rvn_ui::programmable::safe_asset_path(&leaf.path)
                    || leaf
                        .color
                        .iter()
                        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                {
                    return Err("Invalid retired image or color".into());
                }
            }
        }
        RetiredFrame::Music { file, gain } => {
            if !rvn_ui::programmable::safe_asset_path(file)
                || !gain.is_finite()
                || !(0.0..=1.0).contains(gain)
            {
                return Err("Invalid retired music projection".into());
            }
        }
    }
    Ok(())
}

/// Sample all OLD clocks/poses once. Rebuild this transient value when adapter
/// geometry or reduced-motion policy changes. Do not advance any saved clock.
/// The adapter must derive root/layer geometry from this payload's logical
/// source, not a current Entity or replacement with the same character alias.
pub fn freeze_appearance(
    presentation: &RetiredPresentation,
    geometry: Option<&HistoricalGeometry>,
    reduced: bool,
) -> Result<FrozenAppearance, String> {
    presentation.validate_structure()?;
    let (frame, layered) = match &presentation.model {
        RetiredModel::Music { file, volume, fade } => {
            if geometry.is_some() {
                return Err("Music has no visual geometry".into());
            }
            let gain = match fade {
                Some(fade) => effect_sampling::sample_audio(fade.from, fade.to, &fade.clock),
                None => *volume,
            };
            (
                RetiredFrame::Music {
                    file: file.clone(),
                    gain,
                },
                false,
            )
        }
        RetiredModel::Cinematic { id, sample } => {
            let geometry = geometry.ok_or("CG requires historical geometry")?;
            if geometry.source != presentation.original_target.identity || !geometry.layers.is_empty() {
                return Err("Historical CG geometry identity mismatch".into());
            }
            validate_geometry(&geometry.root)?;
            // CG is cover-based: actual adapter geometry may change on resize.
            let (root, alpha) = source_pose(sample, &geometry.root.transform, reduced);
            (RetiredFrame::Visual { root, leaves:vec![RetiredLeaf { layer:None, path:id.clone(),
                pose:TransformSnapshot::default(), color:[1.0,1.0,1.0,alpha as f32] }] }, false)
        }
        RetiredModel::Background { path, sample } => {
            let geometry = geometry.ok_or("Background requires historical geometry")?;
            if geometry.source != presentation.original_target.identity
                || !geometry.layers.is_empty()
            {
                return Err("Historical background geometry identity mismatch".into());
            }
            validate_geometry(&geometry.root)?;
            // A resolved background base has no authored Move/style rebase.
            // Keep its historical capture; unresolved sources use the adapter's
            // logical geometry. Sprite roots deliberately follow another rule.
            let base = sample
                .fade
                .as_ref()
                .filter(|fade| fade.base_resolved)
                .map(|fade| &fade.base)
                .unwrap_or(&geometry.root.transform);
            let (mut root, alpha) = source_pose(sample, base, reduced);
            let mut color = [1.0, 1.0, 1.0, alpha as f32];
            let mut path = path.clone();
            if let Some(motion) = &sample.root_motion {
                let pose = motion.selected(reduced);
                root = effect_sampling::compose_motion(&root, pose, geometry.root.motion);
                color = effect_sampling::motion_color(color, pose);
                if let Some(frame) = &pose.frame {
                    path = frame.clone();
                }
            }
            (
                RetiredFrame::Visual {
                    root,
                    leaves: vec![RetiredLeaf {
                        layer: None,
                        path,
                        pose: TransformSnapshot::default(),
                        color,
                    }],
                },
                false,
            )
        }
        RetiredModel::Sprite {
            id,
            state,
            style,
            composition,
            sample,
        } => {
            let geometry = geometry.ok_or("Sprite requires historical geometry")?;
            if geometry.source != presentation.original_target.identity {
                return Err("Historical sprite geometry identity mismatch".into());
            }
            validate_geometry(&geometry.root)?;
            // C10: logical placement/style of the shared Game sprite wins over
            // an old resolved effect base after Move/SetStyle, even if no frame
            // was projected between mutation and this retirement.
            let base = styled_base(&geometry.root, style)?;
            let (mut root, alpha) = source_pose(sample, &base, reduced);
            let root_tint = tint(style)?;
            let mut leaves = Vec::new();
            if let Some(composition) = composition {
                let selected: Vec<_> = composition
                    .definition
                    .visible_layers(&composition.attributes)
                    .collect();
                let selected_ids: BTreeSet<_> =
                    selected.iter().map(|layer| layer.id.as_str()).collect();
                let geometry_ids: BTreeSet<_> =
                    geometry.layers.keys().map(String::as_str).collect();
                if selected_ids != geometry_ids {
                    return Err("Historical geometry must match selected layers exactly".into());
                }
                // MotionState is a BTreeMap: layer:<id>/<layer> keys precede
                // sprite:<id>. Root color is therefore applied recursively
                // AFTER each leaf's own color, preserving f32 association.
                for layer in selected {
                    let geometry = &geometry.layers[&layer.id];
                    validate_geometry(&geometry.neutral)?;
                    let captured = sample.layers.get(&layer.id);
                    if geometry.source.as_ref()
                        != captured.map(|sample| &sample.original_target.identity)
                    {
                        return Err("Historical layer geometry identity mismatch".into());
                    }
                    let mut local = captured
                        .and_then(|sample| sample.fade.as_ref())
                        .filter(|fade| fade.base_resolved)
                        .map(|fade| fade.base.clone())
                        .unwrap_or_else(|| geometry.neutral.transform.clone());
                    let mut path = layer.image.clone();
                    let mut color = [
                        root_tint[0],
                        root_tint[1],
                        root_tint[2],
                        root_tint[3] * layer.opacity * alpha as f32,
                    ];
                    if let Some(captured) = captured {
                        if let Some(sprite) = &captured.sprite {
                            let base = if sprite.base_resolved {
                                &sprite.base
                            } else {
                                &local
                            };
                            local = effect_sampling::sample_sprite(
                                &sprite.animation,
                                &sprite.clock,
                                base,
                                reduced,
                            );
                        }
                        // Declared C11 extension, also required in Current:
                        // root color/fade -> leaf Sprite/Fade -> leaf/root
                        // Motion. C10 accepted this registry capability but
                        // didn't project a LayeredFade on a non-root leaf.
                        if let Some(fade) = &captured.fade {
                            let sampled = effect_sampling::sample_visual(
                                fade.transition,
                                fade.from,
                                fade.to,
                                &fade.clock,
                                &local,
                                reduced,
                            );
                            color[3] = sampled.with_original_alpha(color[3]);
                            local = sampled.transform;
                        }
                        if let Some(motion) = &captured.motion {
                            let pose = motion.selected(reduced);
                            local = effect_sampling::compose_motion(
                                &local,
                                pose,
                                geometry.neutral.motion,
                            );
                            color = effect_sampling::motion_color(color, pose);
                            if let Some(frame) = &pose.frame {
                                path = frame.clone();
                            }
                        }
                    }
                    leaves.push(RetiredLeaf {
                        layer: Some(layer.id.clone()),
                        path,
                        pose: local,
                        color,
                    });
                }
                if let Some(motion) = &sample.root_motion {
                    let pose = motion.selected(reduced);
                    root = effect_sampling::compose_motion(&root, pose, geometry.root.motion);
                    for leaf in &mut leaves {
                        leaf.color = effect_sampling::motion_color(leaf.color, pose);
                    }
                    // C10 replaces UiImage descendants here, not Sprite leaf
                    // textures. A composition root frame is NOT a leaf frame.
                }
            } else {
                if !geometry.layers.is_empty() {
                    return Err("Simple sprite has no layer geometry".into());
                }
                let mut color = root_tint;
                color[3] = (f64::from(color[3]) * alpha) as f32;
                let mut path = sprite_path(id, state);
                if let Some(motion) = &sample.root_motion {
                    let pose = motion.selected(reduced);
                    root = effect_sampling::compose_motion(&root, pose, geometry.root.motion);
                    color = effect_sampling::motion_color(color, pose);
                    if let Some(frame) = &pose.frame {
                        path = frame.clone();
                    }
                }
                leaves.push(RetiredLeaf {
                    layer: None,
                    path,
                    pose: TransformSnapshot::default(),
                    color,
                });
            }
            (RetiredFrame::Visual { root, leaves }, composition.is_some())
        }
    };
    validate_frame(&frame)?;
    Ok(FrozenAppearance {
        source_bytes: serde_json::to_vec(presentation).map_err(|error| error.to_string())?,
        frame,
        layered,
        reduced,
    })
}

/// Apply only the NEW outgoing clock around the already sampled source pose.
/// No old contribution is sampled twice, and Music gain is multiplied once.
pub fn sample_outgoing(
    frozen: &FrozenAppearance,
    effect: &SavedEffect,
    reduced: bool,
) -> Result<RetiredFrame, String> {
    if reduced != frozen.reduced {
        return Err(
            "Changed reduced-motion policy requires sampling the frozen source again".into(),
        );
    }
    effect.validate()?;
    let presentation = effect
        .presentation
        .as_ref()
        .ok_or("Outgoing payload is missing")?;
    presentation.validate_outgoing(effect)?;
    if serde_json::to_vec(presentation).map_err(|error| error.to_string())? != frozen.source_bytes {
        return Err("Frozen appearance belongs to a different historical payload".into());
    }
    let frame = match (&frozen.frame, &effect.parameters) {
        (RetiredFrame::Music { file, gain }, EffectParameters::AudioFade { .. }) => {
            RetiredFrame::Music {
                file: file.clone(),
                gain: effect_sampling::retired_music_gain(*gain, &effect.clock),
            }
        }
        (
            RetiredFrame::Visual { root, leaves },
            EffectParameters::VisualFade {
                from,
                to,
                transition,
                ..
            },
        ) => {
            let visual = effect_sampling::sample_visual(
                *transition,
                *from,
                *to,
                &effect.clock,
                root,
                reduced,
            );
            let mut leaves = leaves.clone();
            for leaf in &mut leaves {
                leaf.color[3] = if frozen.layered {
                    leaf.color[3] * visual.alpha as f32
                } else {
                    (f64::from(leaf.color[3]) * visual.alpha) as f32
                };
            }
            RetiredFrame::Visual {
                root: visual.transform,
                leaves,
            }
        }
        _ => return Err("Outgoing effect and sampled appearance families disagree".into()),
    };
    validate_frame(&frame)?;
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retired_presentations::{
        FrozenAudioSample, FrozenFadeSample, FrozenLayerSample, FrozenMotionSample,
        FrozenSpriteSample, PresentationSlot,
    };
    use crate::scene_resources::{
        SpriteEffectKind, TargetIdentity, TargetKind, TargetRecord, VisualTransition,
    };
    use crate::scene_tasks::TaskOwner;
    use rvn_ui::motion::Pose;

    fn clock(elapsed: f64, duration: f64) -> EffectClock {
        EffectClock {
            elapsed,
            duration,
            looping: false,
        }
    }
    fn record(kind: TargetKind, alias: &str, incarnation: u64) -> TargetRecord {
        TargetRecord {
            owner: TaskOwner::Game,
            identity: TargetIdentity {
                kind,
                alias: alias.into(),
                incarnation,
            },
        }
    }
    fn sprite() -> RetiredPresentation {
        RetiredPresentation {
            original_slot: PresentationSlot::Sprite { id: "hero".into() },
            original_target: record(TargetKind::Sprite, "hero", 2),
            model: RetiredModel::Sprite {
                id: "hero".into(),
                state: crate::types::SpriteState::new(
                    Some("old".into()),
                    rvn_parser::Position::Right,
                ),
                style: SpriteStyle::default(),
                composition: None,
                sample: FrozenVisualSample::default(),
            },
        }
    }
    fn sample(presentation: &mut RetiredPresentation) -> &mut FrozenVisualSample {
        match &mut presentation.model {
            RetiredModel::Sprite { sample, .. } | RetiredModel::Background { sample, .. } => sample,
            _ => panic!("not visual"),
        }
    }
    fn neutral(x: f32, y: f32) -> NeutralGeometry {
        NeutralGeometry {
            transform: TransformSnapshot {
                translation: [x, y, 0.0],
                ..Default::default()
            },
            motion: MotionGeometry {
                size: [100.0, 50.0],
                factor: [2.0, 2.0],
                ui: false,
            },
        }
    }
    fn geometry(presentation: &RetiredPresentation) -> HistoricalGeometry {
        HistoricalGeometry {
            source: presentation.original_target.identity.clone(),
            root: neutral(50.0, 100.0),
            layers: BTreeMap::new(),
        }
    }
    fn outgoing(
        presentation: &RetiredPresentation,
        elapsed: f64,
        transition: VisualTransition,
    ) -> SavedEffect {
        let layered = matches!(
            &presentation.model,
            RetiredModel::Sprite {
                composition: Some(_),
                ..
            }
        );
        SavedEffect {
            presentation: Some(presentation.clone()),
            base_resolved: false,
            clock: clock(elapsed, 4.0),
            parameters: match &presentation.model {
                RetiredModel::Music { .. } => EffectParameters::AudioFade {
                    from: 1.0,
                    to: 0.0,
                    despawn_on_done: true,
                },
                _ => EffectParameters::VisualFade {
                    from: 1.0,
                    to: 0.0,
                    despawn_on_finish: true,
                    transition,
                    base: TransformSnapshot::default(),
                    layered,
                },
            },
        }
    }
    fn fade(elapsed: f64, from: f32, to: f32, layered: bool) -> FrozenFadeSample {
        FrozenFadeSample {
            from,
            to,
            transition: VisualTransition::Fade,
            clock: clock(elapsed, 4.0),
            base: TransformSnapshot::default(),
            base_resolved: false,
            layered,
        }
    }
    fn motion(pose: Pose) -> FrozenMotionSample {
        FrozenMotionSample {
            reduced_pose: Pose::default(),
            pose,
        }
    }
    fn visual(frame: &RetiredFrame) -> (&TransformSnapshot, &[RetiredLeaf]) {
        match frame {
            RetiredFrame::Visual { root, leaves } => (root, leaves),
            _ => panic!("not visual"),
        }
    }
    fn composed() -> (RetiredPresentation, HistoricalGeometry) {
        let mut presentation = sprite();
        let definition = rvn_ui::composition::Composition::parse(serde_json::json!({
            "size":[400,600],"layers":[
                {"id":"body","image":"images/body.png","opacity":0.3},
                {"id":"eyes","image":"images/eyes.png","opacity":0.8}
            ]
        }))
        .unwrap();
        if let RetiredModel::Sprite { composition, .. } = &mut presentation.model {
            *composition = Some(crate::composition::CharacterComposition {
                definition,
                attributes: Default::default(),
            });
        }
        let mut geometry = geometry(&presentation);
        geometry.layers.insert(
            "body".into(),
            LayerGeometry {
                source: None,
                neutral: neutral(10.0, 20.0),
            },
        );
        geometry.layers.insert(
            "eyes".into(),
            LayerGeometry {
                source: None,
                neutral: neutral(30.0, 40.0),
            },
        );
        (presentation, geometry)
    }

    #[test]
    fn frozen_sprite_then_visual_then_motion_then_outgoing_have_independent_clocks() {
        let mut presentation = sprite();
        let source = sample(&mut presentation);
        source.sprite = Some(FrozenSpriteSample {
            animation: SpriteEffectKind::Bounce { height: 10.0 },
            clock: clock(1.0, 2.0),
            base: TransformSnapshot::default(),
            base_resolved: false,
        });
        source.fade = Some(FrozenFadeSample {
            transition: VisualTransition::SlideLeft,
            ..fade(1.0, 0.0, 1.0, false)
        });
        source.root_motion = Some(motion(Pose {
            x: 7.0,
            y: 3.0,
            ..Default::default()
        }));
        let frozen =
            freeze_appearance(&presentation, Some(&geometry(&presentation)), false).unwrap();
        let (root, _) = visual(&frozen.frame);
        assert_eq!(root.translation, [1504.0, 104.0, 0.0]);
        let result = sample_outgoing(
            &frozen,
            &outgoing(&presentation, 1.0, VisualTransition::SlideRight),
            false,
        )
        .unwrap();
        let (root, leaves) = visual(&result);
        assert_eq!(root.translation, [1024.0, 104.0, 0.0]);
        assert_eq!(leaves[0].path, "sprites/hero/old.png");
        assert_eq!(leaves[0].color[3], 0.1875);
    }

    #[test]
    fn old_clock_bits_and_payload_are_immutable_across_outgoing_sampling() {
        let mut presentation = sprite();
        sample(&mut presentation).sprite = Some(FrozenSpriteSample {
            animation: SpriteEffectKind::Pulse { scale: 1.5 },
            clock: EffectClock {
                duration: 1.0000000000000002,
                elapsed: 7.5,
                looping: true,
            },
            base: TransformSnapshot::default(),
            base_resolved: false,
        });
        let before = serde_json::to_vec(&presentation).unwrap();
        let frozen =
            freeze_appearance(&presentation, Some(&geometry(&presentation)), false).unwrap();
        let initial = frozen.frame.clone();
        for elapsed in [0.0, 0.125, 1.0, 4.0] {
            let effect = outgoing(&presentation, elapsed, VisualTransition::Fade);
            let before_effect = serde_json::to_vec(&effect).unwrap();
            let first = sample_outgoing(&frozen, &effect, false).unwrap();
            assert_eq!(sample_outgoing(&frozen, &effect, false).unwrap(), first);
            assert_eq!(serde_json::to_vec(&effect).unwrap(), before_effect);
            assert_eq!(frozen.frame, initial);
        }
        assert_eq!(serde_json::to_vec(&presentation).unwrap(), before);
    }

    #[test]
    fn logical_move_and_style_win_over_resolved_old_base_before_any_projection() {
        let mut presentation = sprite();
        if let RetiredModel::Sprite { style, .. } = &mut presentation.model {
            *style = SpriteStyle {
                flip_x: true,
                flip_y: false,
                scale: 2.0,
                rotation: 45.0,
                tint: Some("#1234".into()),
            };
        }
        sample(&mut presentation).sprite = Some(FrozenSpriteSample {
            animation: SpriteEffectKind::Pulse { scale: 2.0 },
            clock: clock(1.0, 4.0),
            base: TransformSnapshot {
                translation: [-900.0, -800.0, 0.0],
                scale: [3.0; 3],
                ..Default::default()
            },
            base_resolved: true,
        });
        let mut geometry = geometry(&presentation);
        geometry.root.transform.translation = [80.0, 90.0, 7.0];
        geometry.root.transform.scale[2] = 3.0;
        let frozen = freeze_appearance(&presentation, Some(&geometry), false).unwrap();
        let (root, leaves) = visual(&frozen.frame);
        assert_eq!(root.translation, [80.0, 90.0, 7.0]);
        assert_eq!(root.scale, [-4.0, 4.0, 6.0]);
        assert_eq!(
            leaves[0].color,
            [17.0 / 255.0, 34.0 / 255.0, 51.0 / 255.0, 68.0 / 255.0]
        );
        let half = 45.0f32.to_radians() * 0.5;
        assert_eq!(root.rotation, [0.0, 0.0, half.sin(), half.cos()]);
    }

    #[test]
    fn frozen_pivot_uses_supplied_size_and_world_sign_without_renderer_constants() {
        let mut presentation = sprite();
        sample(&mut presentation).root_motion = Some(motion(Pose {
            rotation: 90.0,
            pivot_x: 1.0,
            pivot_y: 0.5,
            ..Default::default()
        }));
        let mut geometry = geometry(&presentation);
        geometry.root = neutral(10.0, 20.0);
        geometry.root.motion.size = [100.0, 20.0];
        let frozen = freeze_appearance(&presentation, Some(&geometry), false).unwrap();
        let (root, _) = visual(&frozen.frame);
        assert!((root.translation[0] - 60.0).abs() < 0.00002);
        assert!((root.translation[1] - 70.0).abs() < 0.00002);
        assert!(root.rotation[2] < 0.0);
        geometry.root.motion.ui = true;
        assert!(freeze_appearance(&presentation, Some(&geometry), false).is_err());
    }

    #[test]
    fn layered_color_applies_factor_cast_then_leaf_motion_then_root_motion() {
        let (mut presentation, mut geometry) = composed();
        if let RetiredModel::Sprite { style, .. } = &mut presentation.model {
            style.tint = Some("#abcdefcc".into());
        }
        let source = sample(&mut presentation);
        source.fade = Some(fade(1.0, 0.0, 1.0, true));
        source.root_motion = Some(motion(Pose {
            tint_a: 0.9,
            opacity: 0.85,
            tint_r: 0.2,
            ..Default::default()
        }));
        let layer_record = record(TargetKind::Layer, "hero/body", 3);
        source.layers.insert(
            "body".into(),
            FrozenLayerSample {
                original_target: layer_record.clone(),
                sprite: None,
                fade: None,
                motion: Some(motion(Pose {
                    tint_a: 0.7,
                    opacity: 0.6,
                    tint_r: 0.3,
                    x: 4.0,
                    ..Default::default()
                })),
            },
        );
        geometry.layers.get_mut("body").unwrap().source = Some(layer_record.identity);
        let frozen = freeze_appearance(&presentation, Some(&geometry), false).unwrap();
        let (_, leaves) = visual(&frozen.frame);
        let body = &leaves[0];
        let source_alpha = ((204.0f32 / 255.0) * 0.3) * 0.25f32;
        let expected = (source_alpha * (0.7f32 * 0.6)) * (0.9f32 * 0.85);
        assert_eq!(body.color[3].to_bits(), expected.to_bits());
        assert_eq!(
            body.color[0].to_bits(),
            ((171.0f32 / 255.0 * 0.3) * 0.2).to_bits()
        );
        assert_eq!(body.pose.translation, [18.0, 20.0, 0.0]);
        let result = sample_outgoing(
            &frozen,
            &outgoing(&presentation, 1.0, VisualTransition::Fade),
            false,
        )
        .unwrap();
        assert_eq!(
            visual(&result).1[0].color[3].to_bits(),
            (expected * 0.75f32).to_bits()
        );
    }

    #[test]
    fn composition_root_frame_never_overrides_leaf_images_but_leaf_modes_do() {
        let (mut presentation, mut geometry) = composed();
        let source = sample(&mut presentation);
        source.root_motion = Some(FrozenMotionSample {
            pose: Pose {
                frame: Some("images/root-normal.png".into()),
                ..Default::default()
            },
            reduced_pose: Pose {
                frame: Some("images/root-reduced.png".into()),
                ..Default::default()
            },
        });
        let layer_record = record(TargetKind::Layer, "hero/body", 3);
        source.layers.insert(
            "body".into(),
            FrozenLayerSample {
                original_target: layer_record.clone(),
                sprite: None,
                fade: None,
                motion: Some(FrozenMotionSample {
                    pose: Pose {
                        frame: Some("images/body-normal.png".into()),
                        ..Default::default()
                    },
                    reduced_pose: Pose {
                        frame: Some("images/body-reduced.png".into()),
                        ..Default::default()
                    },
                }),
            },
        );
        geometry.layers.get_mut("body").unwrap().source = Some(layer_record.identity);
        for (reduced, expected) in [
            (false, "images/body-normal.png"),
            (true, "images/body-reduced.png"),
        ] {
            let frozen = freeze_appearance(&presentation, Some(&geometry), reduced).unwrap();
            let (_, leaves) = visual(&frozen.frame);
            assert_eq!(leaves[0].path, expected);
            assert_eq!(leaves[1].path, "images/eyes.png");
        }
        let paths = presentation.resource_paths();
        for path in [
            "images/root-normal.png",
            "images/root-reduced.png",
            "images/body-normal.png",
            "images/body-reduced.png",
        ] {
            assert!(paths.contains(path));
        }
    }

    #[test]
    fn exact_historical_geometry_refuses_homonym_and_missing_or_extra_layer() {
        let (presentation, geometry) = composed();
        freeze_appearance(&presentation, Some(&geometry), false).unwrap();
        let mut wrong = geometry.clone();
        wrong.source.incarnation += 1;
        assert!(freeze_appearance(&presentation, Some(&wrong), false).is_err());
        wrong = geometry.clone();
        wrong.layers.remove("eyes");
        assert!(freeze_appearance(&presentation, Some(&wrong), false).is_err());
        wrong = geometry.clone();
        wrong.layers.insert(
            "child-only".into(),
            LayerGeometry {
                source: None,
                neutral: neutral(0.0, 0.0),
            },
        );
        assert!(freeze_appearance(&presentation, Some(&wrong), false).is_err());
        wrong = geometry.clone();
        wrong.layers.get_mut("body").unwrap().source =
            Some(record(TargetKind::Layer, "hero/body", 3).identity);
        assert!(freeze_appearance(&presentation, Some(&wrong), false).is_err());
    }

    #[test]
    fn music_freezes_absolute_source_gain_and_multiplies_outgoing_factor_once() {
        let presentation = RetiredPresentation {
            original_slot: PresentationSlot::Music,
            original_target: record(TargetKind::Music, "music", 5),
            model: RetiredModel::Music {
                file: "audio/old.ogg".into(),
                volume: 0.05,
                fade: Some(FrozenAudioSample {
                    from: 0.1,
                    to: 0.8,
                    clock: clock(1.0, 4.0),
                }),
            },
        };
        let frozen = freeze_appearance(&presentation, None, false).unwrap();
        let effect = outgoing(&presentation, 2.0, VisualTransition::Fade);
        let result = sample_outgoing(&frozen, &effect, false).unwrap();
        let absolute = (f64::from(0.1f32) + (f64::from(0.8f32) - f64::from(0.1f32)) * 0.25) as f32;
        assert_eq!(
            result,
            RetiredFrame::Music {
                file: "audio/old.ogg".into(),
                gain: (f64::from(absolute) * 0.5) as f32
            }
        );
        assert_eq!(
            presentation
                .outgoing_music_gain(&effect)
                .unwrap()
                .unwrap()
                .to_bits(),
            ((f64::from(absolute) * 0.5f64) as f32).to_bits()
        );
    }

    #[test]
    fn reduced_policy_uses_saved_base_pose_and_does_not_change_any_clock() {
        let mut presentation = sprite();
        let source = sample(&mut presentation);
        source.root_motion = Some(FrozenMotionSample {
            pose: Pose {
                x: 20.0,
                ..Default::default()
            },
            reduced_pose: Pose {
                x: 3.0,
                ..Default::default()
            },
        });
        source.sprite = Some(FrozenSpriteSample {
            animation: SpriteEffectKind::Bounce { height: 8.0 },
            clock: clock(1.0, 2.0),
            base: Default::default(),
            base_resolved: false,
        });
        let before = serde_json::to_vec(&presentation).unwrap();
        let normal =
            freeze_appearance(&presentation, Some(&geometry(&presentation)), false).unwrap();
        let reduced =
            freeze_appearance(&presentation, Some(&geometry(&presentation)), true).unwrap();
        assert_eq!(visual(&normal.frame).0.translation, [90.0, 108.0, 0.0]);
        assert_eq!(visual(&reduced.frame).0.translation, [56.0, 100.0, 0.0]);
        let effect = outgoing(&presentation, 1.0, VisualTransition::SlideLeft);
        let before_effect = serde_json::to_vec(&effect).unwrap();
        assert!(sample_outgoing(&normal, &effect, true).is_err());
        let terminal = sample_outgoing(&reduced, &effect, true).unwrap();
        assert_eq!(visual(&terminal).1[0].color[3], 0.0);
        assert_eq!(serde_json::to_vec(&effect).unwrap(), before_effect);
        assert_eq!(serde_json::to_vec(&presentation).unwrap(), before);
    }

    #[test]
    fn background_resolved_capture_is_preserved_and_unresolved_uses_frozen_geometry() {
        let mut presentation = RetiredPresentation {
            original_slot: PresentationSlot::Background,
            original_target: record(TargetKind::Background, "background", 1),
            model: RetiredModel::Background {
                path: "images/old-bg.png".into(),
                sample: Default::default(),
            },
        };
        sample(&mut presentation).fade = Some(FrozenFadeSample {
            base: TransformSnapshot {
                translation: [8.0, 9.0, 0.0],
                ..Default::default()
            },
            base_resolved: true,
            ..fade(1.0, 0.0, 1.0, false)
        });
        let geometry = geometry(&presentation);
        let captured = freeze_appearance(&presentation, Some(&geometry), false).unwrap();
        assert_eq!(visual(&captured.frame).0.translation, [8.0, 9.0, 0.0]);
        sample(&mut presentation)
            .fade
            .as_mut()
            .unwrap()
            .base_resolved = false;
        let unresolved = freeze_appearance(&presentation, Some(&geometry), false).unwrap();
        assert_eq!(visual(&unresolved.frame).0.translation, [50.0, 100.0, 0.0]);
    }

    #[test]
    fn wrong_payload_family_and_nonfinite_geometry_are_refused_without_mutation() {
        let presentation = sprite();
        let frozen =
            freeze_appearance(&presentation, Some(&geometry(&presentation)), false).unwrap();
        let before = frozen.frame.clone();
        let mut effect = outgoing(&presentation, 1.0, VisualTransition::Fade);
        if let Some(RetiredPresentation {
            model: RetiredModel::Sprite { state, .. },
            ..
        }) = &mut effect.presentation
        {
            state.emotion = Some("child".into());
        }
        assert!(sample_outgoing(&frozen, &effect, false).is_err());
        effect = outgoing(&presentation, 1.0, VisualTransition::Fade);
        effect.parameters = EffectParameters::AudioFade {
            from: 1.0,
            to: 0.0,
            despawn_on_done: true,
        };
        assert!(sample_outgoing(&frozen, &effect, false).is_err());
        let mut invalid = geometry(&presentation);
        invalid.root.motion.factor[0] = f32::NAN;
        assert!(freeze_appearance(&presentation, Some(&invalid), false).is_err());
        invalid = geometry(&presentation);
        invalid.root.transform.rotation[3] = 1.1;
        assert!(freeze_appearance(&presentation, Some(&invalid), false).is_err());
        assert_eq!(frozen.frame, before);
    }

    #[test]
    fn simple_alpha_is_single_final_cast_and_oversized_style_is_a_projection_error() {
        let mut presentation = sprite();
        if let RetiredModel::Sprite { style, .. } = &mut presentation.model {
            style.tint = Some("#abcdefab".into());
        }
        sample(&mut presentation).fade = Some(FrozenFadeSample {
            clock: clock(1.0, 3.0),
            ..fade(0.0, 0.1, 0.8, false)
        });
        let frozen =
            freeze_appearance(&presentation, Some(&geometry(&presentation)), false).unwrap();
        let alpha = f64::from(0.1f32) + (f64::from(0.8f32) - f64::from(0.1f32)) / 3.0;
        let expected = (f64::from(171.0f32 / 255.0) * alpha) as f32;
        assert_eq!(
            visual(&frozen.frame).1[0].color[3].to_bits(),
            expected.to_bits()
        );
        if let RetiredModel::Sprite { style, .. } = &mut presentation.model {
            style.scale = f32::MAX;
        }
        presentation.validate_structure().unwrap();
        assert!(freeze_appearance(&presentation, Some(&geometry(&presentation)), false).is_err());
    }

    #[test]
    fn leaf_fade_extension_composes_after_root_factor_before_motion_without_accumulation() {
        let (mut presentation, mut geometry) = composed();
        let source = sample(&mut presentation);
        source.fade = Some(fade(1.0, 0.0, 1.0, true));
        let layer_record = record(TargetKind::Layer, "hero/body", 3);
        source.layers.insert(
            "body".into(),
            FrozenLayerSample {
                original_target: layer_record.clone(),
                sprite: None,
                fade: Some(FrozenFadeSample {
                    from: 0.1,
                    to: 0.8,
                    transition: VisualTransition::SlideRight,
                    clock: clock(1.0, 3.0),
                    base_resolved: true,
                    layered: true,
                    base: TransformSnapshot {
                        translation: [25.0, 30.0, 0.0],
                        ..Default::default()
                    },
                }),
                motion: Some(motion(Pose {
                    x: 4.0,
                    tint_a: 0.5,
                    opacity: 0.75,
                    ..Default::default()
                })),
            },
        );
        geometry.layers.get_mut("body").unwrap().source = Some(layer_record.identity);
        let bytes = serde_json::to_vec(&presentation).unwrap();
        let frozen = freeze_appearance(&presentation, Some(&geometry), false).unwrap();
        assert_eq!(
            freeze_appearance(&presentation, Some(&geometry), false)
                .unwrap()
                .frame,
            frozen.frame
        );
        let (_, leaves) = visual(&frozen.frame);
        let leaf_factor = f64::from(0.1f32) + (f64::from(0.8f32) - f64::from(0.1f32)) / 3.0;
        let root_alpha = 1.0f32 * 0.3 * 0.25f32;
        let leaf_alpha = (f64::from(root_alpha) * leaf_factor) as f32;
        assert_eq!(
            leaves[0].color[3].to_bits(),
            (leaf_alpha * (0.5f32 * 0.75)).to_bits()
        );
        // 25 - 1920*(1-1/3) + 4*2 = -1247. The other layer is intact.
        assert_eq!(leaves[0].pose.translation, [-1247.0, 30.0, 0.0]);
        assert_eq!(leaves[1].pose.translation, [30.0, 40.0, 0.0]);
        assert_eq!(leaves[1].color[3], 0.8f32 * 0.25);
        assert_eq!(serde_json::to_vec(&presentation).unwrap(), bytes);
        let reduced = freeze_appearance(&presentation, Some(&geometry), true).unwrap();
        assert_eq!(
            visual(&reduced.frame).1[0].pose.translation,
            [25.0, 30.0, 0.0]
        );
        assert_eq!(
            visual(&reduced.frame).1[0].color[3].to_bits(),
            (((1.0f32 * 0.3) as f64 * f64::from(0.8f32)) as f32).to_bits()
        );
    }
}
