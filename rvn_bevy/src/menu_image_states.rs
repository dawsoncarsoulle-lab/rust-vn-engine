//! State images share the same selection rules as the authoring composition.
use super::*;

#[derive(Component)]
pub(super) struct ImageAppearance {
    owner: Entity,
    normal: Option<String>,
    states: rvn_ui::StateImages,
    handles: std::collections::BTreeMap<String, Handle<Image>>,
    fit: rvn_ui::ImageFit,
    enabled: bool,
    selected: bool,
}
pub(super) fn spawn(
    commands: &mut Commands,
    owner: Entity,
    e: &Element,
    enabled: bool,
    selected: bool,
    assets: &AssetServer,
    ctx: &Context,
) {
    let handles = e
        .asset
        .iter()
        .chain(e.appearance.image_states.paths())
        .map(|path| {
            let handle = ctx
                .thumbnails
                .handles
                .get(path)
                .cloned()
                .unwrap_or_else(|| assets.load(path.clone()));
            (path.clone(), handle)
        })
        .collect();
    let mut image = commands.spawn((
        ImageBundle {
            style: Style {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            visibility: Visibility::Hidden,
            image: UiImage {
                color: Color::srgba(1.0, 1.0, 1.0, e.appearance.opacity),
                ..default()
            },
            ..default()
        },
        ImageAppearance {
            owner,
            normal: e.asset.clone(),
            states: e.appearance.image_states.clone(),
            handles,
            fit: e.layout_options.image_fit,
            enabled,
            selected,
        },
    ));
    if e.layout_options.image_fit == rvn_ui::ImageFit::NineSlice {
        let [left, top, right, bottom] = e.layout_options.slice;
        image.insert(bevy::sprite::ImageScaleMode::Sliced(
            bevy::sprite::TextureSlicer {
                border: bevy::sprite::BorderRect {
                    left,
                    top,
                    right,
                    bottom,
                },
                ..default()
            },
        ));
    }
    let image = image.id();
    if e.layout_options.image_fit == rvn_ui::ImageFit::Cover {
        // Clip the background, not the panel's authored children or its shadow.
        let clip = commands
            .spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                ..default()
            })
            .id();
        commands.entity(clip).add_child(image);
        commands.entity(owner).add_child(clip);
    } else {
        commands.entity(owner).add_child(image);
    }
}
pub(super) fn update(
    mut images: Query<(&ImageAppearance, &mut UiImage, &mut Style, &mut Visibility)>,
    nodes: Query<&Node>,
    parents: Query<&Parent>,
    interactive: Query<(
        Option<&Interaction>,
        Option<&Outline>,
        Option<&crate::components::ChoiceButton>,
    )>,
    choices: Res<ChoiceFocus>,
    loaded: Res<Assets<Image>>,
) {
    for (appearance, mut image, mut style, mut visibility) in &mut images {
        let mut node = appearance.owner;
        let (mut pressed, mut hover, mut focus) = (false, false, false);
        for _ in 0..64 {
            if let Ok((interaction, outline, choice)) = interactive.get(node) {
                focus |= outline.is_some() || choice.is_some_and(|c| choices.0 == Some(c.0));
                if let Some(interaction) = interaction {
                    pressed = *interaction == Interaction::Pressed;
                    hover = *interaction == Interaction::Hovered;
                    break;
                }
            }
            let Ok(parent) = parents.get(node) else { break };
            node = parent.get();
        }
        let selected = appearance
            .states
            .resolve(
                appearance.normal.as_deref(),
                appearance.enabled,
                pressed,
                hover,
                focus,
                appearance.selected,
            )
            .and_then(|path| appearance.handles.get(path));
        let Some(handle) = selected else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
        if image.texture != *handle {
            image.texture = handle.clone();
        }
        if appearance.fit == rvn_ui::ImageFit::NineSlice {
            continue;
        }
        let Ok(parent) = nodes.get(appearance.owner) else {
            continue;
        };
        let bounds = parent.size();
        let Some(source) = loaded.get(handle) else {
            continue;
        };
        let dimensions = source.size().as_vec2();
        let size = if appearance.fit == rvn_ui::ImageFit::Stretch {
            bounds
        } else {
            let ratios = bounds / dimensions.max(Vec2::ONE);
            dimensions
                * if appearance.fit == rvn_ui::ImageFit::Cover {
                    ratios.max_element()
                } else {
                    ratios.min_element()
                }
        };
        let (left, top, width, height) = (
            Val::Px((bounds.x - size.x) * 0.5),
            Val::Px((bounds.y - size.y) * 0.5),
            Val::Px(size.x),
            Val::Px(size.y),
        );
        if (style.left, style.top, style.width, style.height) != (left, top, width, height) {
            style.left = left;
            style.top = top;
            style.width = width;
            style.height = height;
        }
    }
}
