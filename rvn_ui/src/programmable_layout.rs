//! Deterministic reference-pixel layout shared by the screen designer and
//! desktop/Web renderers. Text uses a predictable intrinsic box; authors may
//! set width/height for exact composition independent of installed fonts.
use crate::programmable::{Alignment, Component, ComponentKind as Kind, Justification};

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentRect {
    pub id: String,
    pub rect: [f32; 4],
}

fn control(node: &Component) -> bool {
    node.is_control() || node.kind == Kind::Button
}
fn padding(node: &Component) -> f32 {
    if control(node) && node.padding == 0.0 {
        10.0
    } else {
        node.padding
    }
}
fn constrain(value: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    value.max(min.unwrap_or(0.0)).min(max.unwrap_or(16384.0))
}
fn measure(node: &Component, available: [f32; 2]) -> [f32; 2] {
    if let Some(rect) = node.rect {
        return [rect[2], rect[3]];
    }
    let pad = padding(node) * 2.0;
    let inner = [(available[0] - pad).max(0.0), (available[1] - pad).max(0.0)];
    let mut size = match node.kind {
        Kind::Image => [180.0, 180.0],
        Kind::Canvas => [300.0, 180.0],
        Kind::Panel | Kind::Row | Kind::Column => {
            let mut size: [f32; 2] = [0.0, 0.0];
            let mut count = 0;
            for child in node
                .children
                .iter()
                .filter(|child| child.visible && child.rect.is_none())
            {
                let child_size = measure(child, inner);
                let child_size = [
                    child_size[0] + child.margin[1] + child.margin[3],
                    child_size[1] + child.margin[0] + child.margin[2],
                ];
                if node.kind == Kind::Row {
                    size[0] += child_size[0];
                    size[1] = size[1].max(child_size[1]);
                } else {
                    size[1] += child_size[1];
                    size[0] = size[0].max(child_size[0]);
                }
                count += 1;
            }
            if count > 1 {
                size[if node.kind == Kind::Row { 0 } else { 1 }] +=
                    node.spacing * (count - 1) as f32;
            }
            [size[0] + pad, size[1] + pad]
        }
        _ => {
            let text = if node.text.is_empty() {
                &node.placeholder
            } else {
                &node.text
            };
            let lines = text.lines().count().max(1);
            let chars = text
                .lines()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0);
            let width = chars as f32 * node.font_size * 0.56 + pad;
            [width, (lines as f32 * node.font_size * 1.35) + pad]
        }
    };
    if control(node) {
        size[0] = size[0].max(180.0);
        size[1] = size[1].max(44.0);
    }
    size[0] = constrain(
        node.width.unwrap_or(size[0]),
        node.min_width,
        node.max_width,
    );
    size[1] = constrain(
        node.height.unwrap_or(size[1]),
        node.min_height,
        node.max_height,
    );
    size
}

/// Coordinates are absolute in the reference viewport. Authored `rect` values
/// on nested components are relative to the parent's outer top-left corner.
/// Scroll content may extend outside its parent's rectangle and is clipped by
/// the renderer. Hidden children occupy no space and are omitted.
pub fn layout_rects(root: &Component, viewport: [f32; 2]) -> Result<Vec<ComponentRect>, String> {
    root.validate()?;
    if viewport
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0 || *value > 16384.0)
    {
        return Err("Invalid screen layout viewport".into());
    }
    if !root.visible {
        return Ok(Vec::new());
    }
    let absolute_root = root.rect.is_none()
        && root.width.is_none()
        && root.height.is_none()
        && !root.children.is_empty()
        && root
            .children
            .iter()
            .filter(|child| child.visible)
            .all(|child| child.rect.is_some());
    let mut size = if absolute_root {
        viewport
    } else {
        measure(root, viewport)
    };
    if !absolute_root
        && root.rect.is_none()
        && matches!(root.kind, Kind::Panel | Kind::Row | Kind::Column)
    {
        size[0] = size[0].min((viewport[0] - 32.0).max(1.0));
        size[1] = size[1].min((viewport[1] - 32.0).max(1.0));
    }
    let rect = root.rect.unwrap_or([
        (viewport[0] - size[0]) * 0.5,
        (viewport[1] - size[1]) * 0.5,
        size[0],
        size[1],
    ]);
    let mut result = Vec::new();
    fn place(node: &Component, rect: [f32; 4], result: &mut Vec<ComponentRect>) {
        if !node.visible {
            return;
        }
        result.push(ComponentRect {
            id: node.id.clone(),
            rect,
        });
        let pad = padding(node);
        let inner = [
            (rect[2] - pad * 2.0).max(0.0),
            (rect[3] - pad * 2.0).max(0.0),
        ];
        let row = node.kind == Kind::Row;
        let axis = if row { 0 } else { 1 };
        let cross = 1 - axis;
        let children: Vec<_> = node
            .children
            .iter()
            .filter(|child| child.visible && child.rect.is_none())
            .collect();
        let mut dimensions = Vec::new();
        let mut occupied = 0.0;
        for child in &children {
            let mut size = measure(child, inner);
            if node.align == Alignment::Stretch
                && if row {
                    child.height.is_none()
                } else {
                    child.width.is_none()
                }
            {
                size[cross] = (inner[cross]
                    - if row {
                        child.margin[0] + child.margin[2]
                    } else {
                        child.margin[1] + child.margin[3]
                    })
                .max(0.0);
                size[cross] = if row {
                    constrain(size[cross], child.min_height, child.max_height)
                } else {
                    constrain(size[cross], child.min_width, child.max_width)
                };
            }
            occupied += size[axis]
                + if row {
                    child.margin[1] + child.margin[3]
                } else {
                    child.margin[0] + child.margin[2]
                };
            dimensions.push(size);
        }
        occupied += node.spacing * children.len().saturating_sub(1) as f32;
        let free = (inner[axis] - occupied).max(0.0);
        let count = children.len() as f32;
        let (mut cursor, gap) = match node.justify {
            Justification::Start => (0.0, node.spacing),
            Justification::Center => (free * 0.5, node.spacing),
            Justification::End => (free, node.spacing),
            Justification::SpaceBetween => (
                0.0,
                node.spacing
                    + if count > 1.0 {
                        free / (count - 1.0)
                    } else {
                        0.0
                    },
            ),
            Justification::SpaceAround => (
                if count > 0.0 { free / count * 0.5 } else { 0.0 },
                node.spacing + if count > 0.0 { free / count } else { 0.0 },
            ),
            Justification::SpaceEvenly => {
                (free / (count + 1.0), node.spacing + free / (count + 1.0))
            }
        };
        let mut wrap_cross = 0.0;
        let mut line_cross: f32 = 0.0;
        for (child, size) in children.into_iter().zip(dimensions) {
            let leading = if row {
                child.margin[3]
            } else {
                child.margin[0]
            };
            let trailing = if row {
                child.margin[1]
            } else {
                child.margin[2]
            };
            if node.wrap && cursor > 0.0 && cursor + leading + size[axis] + trailing > inner[axis] {
                cursor = 0.0;
                wrap_cross += line_cross + node.spacing;
                line_cross = 0.0;
            }
            let cross_lead = if row {
                child.margin[0]
            } else {
                child.margin[3]
            };
            let cross_trail = if row {
                child.margin[2]
            } else {
                child.margin[1]
            };
            let cross_free = (inner[cross] - size[cross] - cross_lead - cross_trail).max(0.0);
            let cross_position = cross_lead
                + match node.align {
                    Alignment::Center => cross_free * 0.5,
                    Alignment::End => cross_free,
                    _ => 0.0,
                }
                + wrap_cross;
            let point = if row {
                [cursor + leading, cross_position]
            } else {
                [cross_position, cursor + leading]
            };
            place(
                child,
                [
                    rect[0] + pad + point[0],
                    rect[1] + pad + point[1],
                    size[0],
                    size[1],
                ],
                result,
            );
            cursor += leading + size[axis] + trailing + gap;
            line_cross = line_cross.max(size[cross] + cross_lead + cross_trail);
        }
        for child in node
            .children
            .iter()
            .filter(|child| child.visible && child.rect.is_some())
        {
            let child_rect = child.rect.unwrap();
            place(
                child,
                [
                    rect[0] + child_rect[0],
                    rect[1] + child_rect[1],
                    child_rect[2],
                    child_rect[3],
                ],
                result,
            );
        }
    }
    place(root, rect, &mut result);
    Ok(result)
}
