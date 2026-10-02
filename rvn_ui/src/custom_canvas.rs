//! Portable, bounded drawing commands for creator-defined RVN components.
//! Coordinates are local reference pixels; no renderer or system handles enter
//! this description. Rendering and hit testing share the same affine geometry.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_CANVASES: usize = 32;
pub const MAX_DRAW_PRIMITIVES: usize = 1024;
pub const MAX_GLOBAL_DRAW_PRIMITIVES: usize = 4096;
pub const MAX_DRAW_DEPTH: usize = 16;
pub const MAX_DRAW_POINTS: usize = 256;
pub const MAX_DRAW_TEXT_BYTES: usize = 262_144;
pub const IDENTITY_MATRIX: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
pub type CanvasAffine = [f32; 6];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CanvasPrimitive {
    Rect {
        rect: [f32; 4],
        color: [f32; 4],
        radius: f32,
    },
    Ellipse {
        rect: [f32; 4],
        color: [f32; 4],
    },
    Line {
        points: Vec<[f32; 2]>,
        color: [f32; 4],
        width: f32,
    },
    Polygon {
        points: Vec<[f32; 2]>,
        color: [f32; 4],
    },
    Text {
        text: String,
        position: [f32; 2],
        color: [f32; 4],
        size: f32,
    },
    Image {
        asset: String,
        rect: [f32; 4],
    },
    Group {
        /// Translation x/y, scale x/y, rotation in degrees, opacity.
        transform: [f32; 6],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        clip: Option<[f32; 4]>,
        children: Vec<CanvasPrimitive>,
    },
    Hit {
        id: String,
        rect: [f32; 4],
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CanvasDrawing {
    pub primitives: Vec<CanvasPrimitive>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasFrame {
    pub width: f32,
    pub height: f32,
    pub time: f64,
}

/// A transformed rectangular clip; multiple clips form an intersection.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasClip {
    pub rect: [f32; 4],
    pub matrix: [f32; 6],
}

/// A leaf command with already composed transforms and opacity. Groups never
/// appear in this list. Hit regions are geometry, not rendered decorations.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasItem {
    pub primitive: CanvasPrimitive,
    pub matrix: [f32; 6],
    pub opacity: f32,
    pub clips: Vec<CanvasClip>,
}

fn coordinate(number: f32) -> bool {
    number.is_finite() && number.abs() <= 16_384.0
}
fn rectangle(rect: &[f32; 4]) -> bool {
    rect.iter().all(|n| coordinate(*n)) && rect[2] >= 0.0 && rect[3] >= 0.0
}
fn color(color: &[f32; 4]) -> bool {
    color
        .iter()
        .all(|n| n.is_finite() && (0.0..=1.0).contains(n))
}

impl CanvasDrawing {
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        let primitives = serde_json::from_value(value).map_err(|error| {
            format!("Canvas drawing must return a list of valid primitives: {error}")
        })?;
        let drawing = Self { primitives };
        drawing.validate()?;
        Ok(drawing)
    }

    pub fn validate(&self) -> Result<(), String> {
        fn visit(
            nodes: &[CanvasPrimitive],
            depth: usize,
            count: &mut usize,
            text_bytes: &mut usize,
            hits: &mut BTreeSet<String>,
        ) -> Result<(), String> {
            if depth > MAX_DRAW_DEPTH {
                return Err("Canvas drawing exceeds 16 group levels".into());
            }
            for primitive in nodes {
                *count += 1;
                if *count > MAX_DRAW_PRIMITIVES {
                    return Err("Canvas drawing exceeds 1024 primitives".into());
                }
                let valid = match primitive {
                    CanvasPrimitive::Rect {
                        rect,
                        color: rgba,
                        radius,
                    } => {
                        rectangle(rect)
                            && color(rgba)
                            && radius.is_finite()
                            && (0.0..=16_384.0).contains(radius)
                    }
                    CanvasPrimitive::Ellipse { rect, color: rgba } => {
                        rectangle(rect) && color(rgba)
                    }
                    CanvasPrimitive::Line {
                        points,
                        color: rgba,
                        width,
                    } => {
                        points.len() >= 2
                            && points.len() <= MAX_DRAW_POINTS
                            && points.iter().flatten().all(|n| coordinate(*n))
                            && color(rgba)
                            && width.is_finite()
                            && (0.0..=16_384.0).contains(width)
                    }
                    CanvasPrimitive::Polygon {
                        points,
                        color: rgba,
                    } => {
                        points.len() >= 3
                            && points.len() <= MAX_DRAW_POINTS
                            && points.iter().flatten().all(|n| coordinate(*n))
                            && color(rgba)
                            && simple_polygon(points)
                    }
                    CanvasPrimitive::Text {
                        text,
                        position,
                        color: rgba,
                        size,
                    } => {
                        *text_bytes = text_bytes
                            .checked_add(text.len())
                            .ok_or("Canvas text size overflow")?;
                        text.len() <= 65_536
                            && *text_bytes <= MAX_DRAW_TEXT_BYTES
                            && position.iter().all(|n| coordinate(*n))
                            && color(rgba)
                            && size.is_finite()
                            && (0.0..=16_384.0).contains(size)
                            && *size > 0.0
                    }
                    CanvasPrimitive::Image { asset, rect } => {
                        crate::programmable::safe_asset_path(asset) && rectangle(rect)
                    }
                    CanvasPrimitive::Hit { id, rect } => {
                        !id.is_empty()
                            && id.len() <= 128
                            && hits.insert(id.clone())
                            && rectangle(rect)
                    }
                    CanvasPrimitive::Group {
                        transform,
                        clip,
                        children,
                    } => {
                        if !transform[0..2].iter().all(|n| coordinate(*n))
                            || !transform[2..4]
                                .iter()
                                .all(|n| n.is_finite() && n.abs() <= 64.0)
                            || !transform[4].is_finite()
                            || transform[4].abs() > 360_000.0
                            || !transform[5].is_finite()
                            || !(0.0..=1.0).contains(&transform[5])
                            || clip.as_ref().is_some_and(|rect| !rectangle(rect))
                        {
                            return Err("Invalid canvas group transform or clip".into());
                        }
                        visit(children, depth + 1, count, text_bytes, hits)?;
                        true
                    }
                };
                if !valid {
                    return Err(format!("Invalid canvas primitive geometry, color, resource or identity: {primitive:?}"));
                }
            }
            Ok(())
        }
        visit(&self.primitives, 0, &mut 0, &mut 0, &mut BTreeSet::new())?;
        // Compounded group transforms have their own finite GPU-safe bounds.
        flatten_validated(&self.primitives, IDENTITY_MATRIX, 1.0, &[], &mut Vec::new())?;
        Ok(())
    }

    pub fn primitive_count(&self) -> usize {
        fn count(items: &[CanvasPrimitive]) -> usize {
            items
                .iter()
                .map(|item| {
                    1 + if let CanvasPrimitive::Group { children, .. } = item {
                        count(children)
                    } else {
                        0
                    }
                })
                .sum()
        }
        count(&self.primitives)
    }
}

pub fn transform_point(matrix: [f32; 6], point: [f32; 2]) -> [f32; 2] {
    [
        matrix[0] * point[0] + matrix[2] * point[1] + matrix[4],
        matrix[1] * point[0] + matrix[3] * point[1] + matrix[5],
    ]
}

pub fn inverse_point(matrix: [f32; 6], point: [f32; 2]) -> Option<[f32; 2]> {
    let determinant = matrix[0] * matrix[3] - matrix[1] * matrix[2];
    if !determinant.is_finite() || determinant.abs() < 1.0e-8 {
        return None;
    }
    let x = point[0] - matrix[4];
    let y = point[1] - matrix[5];
    Some([
        (matrix[3] * x - matrix[2] * y) / determinant,
        (-matrix[1] * x + matrix[0] * y) / determinant,
    ])
}

pub fn multiply_matrix(parent: [f32; 6], child: [f32; 6]) -> [f32; 6] {
    [
        parent[0] * child[0] + parent[2] * child[1],
        parent[1] * child[0] + parent[3] * child[1],
        parent[0] * child[2] + parent[2] * child[3],
        parent[1] * child[2] + parent[3] * child[3],
        parent[0] * child[4] + parent[2] * child[5] + parent[4],
        parent[1] * child[4] + parent[3] * child[5] + parent[5],
    ]
}

pub fn group_matrix(transform: [f32; 6]) -> [f32; 6] {
    let angle = transform[4].to_radians();
    let (sin, cos) = angle.sin_cos();
    [
        cos * transform[2],
        sin * transform[2],
        -sin * transform[3],
        cos * transform[3],
        transform[0],
        transform[1],
    ]
}

fn flatten_validated(
    nodes: &[CanvasPrimitive],
    matrix: [f32; 6],
    opacity: f32,
    clips: &[CanvasClip],
    result: &mut Vec<CanvasItem>,
) -> Result<(), String> {
    if matrix
        .iter()
        .any(|n| !n.is_finite() || n.abs() > 1_000_000.0)
    {
        return Err("Compounded canvas transform exceeds safe drawing bounds".into());
    }
    for primitive in nodes {
        if let CanvasPrimitive::Group {
            transform,
            clip,
            children,
        } = primitive
        {
            let matrix = multiply_matrix(matrix, group_matrix(*transform));
            let mut clips = clips.to_vec();
            if let Some(rect) = clip {
                clips.push(CanvasClip {
                    rect: *rect,
                    matrix,
                });
            }
            flatten_validated(children, matrix, opacity * transform[5], &clips, result)?;
        } else {
            result.push(CanvasItem {
                primitive: primitive.clone(),
                matrix,
                opacity,
                clips: clips.to_vec(),
            });
        }
    }
    Ok(())
}

pub fn flatten(drawing: &CanvasDrawing) -> Result<Vec<CanvasItem>, String> {
    drawing.validate()?;
    let mut result = Vec::new();
    flatten_validated(&drawing.primitives, IDENTITY_MATRIX, 1.0, &[], &mut result)?;
    Ok(result)
}

pub fn point_in_rect(point: [f32; 2], rect: [f32; 4]) -> bool {
    point.iter().all(|n| n.is_finite())
        && rect[2] > 0.0
        && rect[3] > 0.0
        && point[0] >= rect[0]
        && point[1] >= rect[1]
        && point[0] <= rect[0] + rect[2]
        && point[1] <= rect[1] + rect[3]
}

pub fn point_in_clips(point: [f32; 2], clips: &[CanvasClip]) -> bool {
    clips.iter().all(|clip| {
        inverse_point(clip.matrix, point).is_some_and(|local| point_in_rect(local, clip.rect))
    })
}

/// The last visible region wins, matching painter order. The host additionally
/// clips events to the canvas/ancestor/scroll viewport and performs capture.
pub fn hit_test(items: &[CanvasItem], point: [f32; 2]) -> Option<String> {
    items.iter().rev().find_map(|item| {
        let CanvasPrimitive::Hit { id, rect } = &item.primitive else {
            return None;
        };
        (item.opacity > 0.0
            && point_in_clips(point, &item.clips)
            && inverse_point(item.matrix, point).is_some_and(|local| point_in_rect(local, *rect)))
        .then(|| id.clone())
    })
}

fn cross(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f64 {
    (f64::from(b[0]) - f64::from(a[0])) * (f64::from(c[1]) - f64::from(a[1]))
        - (f64::from(b[1]) - f64::from(a[1])) * (f64::from(c[0]) - f64::from(a[0]))
}
fn simple_polygon(points: &[[f32; 2]]) -> bool {
    if points
        .iter()
        .enumerate()
        .any(|(index, point)| points[index + 1..].contains(point))
    {
        return false;
    }
    let area: f64 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| f64::from(a[0]) * f64::from(b[1]) - f64::from(b[0]) * f64::from(a[1]))
        .sum();
    if area.abs() < 1.0e-8 {
        return false;
    }
    for a in 0..points.len() {
        let b = (a + 1) % points.len();
        for c in a + 1..points.len() {
            let d = (c + 1) % points.len();
            if a == c || a == d || b == c || b == d {
                continue;
            }
            let x = cross(points[a], points[b], points[c]);
            let y = cross(points[a], points[b], points[d]);
            let u = cross(points[c], points[d], points[a]);
            let v = cross(points[c], points[d], points[b]);
            let on_segment = |a: [f32; 2], b: [f32; 2], p: [f32; 2]| {
                p[0] >= a[0].min(b[0])
                    && p[0] <= a[0].max(b[0])
                    && p[1] >= a[1].min(b[1])
                    && p[1] <= a[1].max(b[1])
            };
            if (x * y < 0.0 && u * v < 0.0)
                || (x == 0.0 && on_segment(points[a], points[b], points[c]))
                || (y == 0.0 && on_segment(points[a], points[b], points[d]))
                || (u == 0.0 && on_segment(points[c], points[d], points[a]))
                || (v == 0.0 && on_segment(points[c], points[d], points[b]))
            {
                return false;
            }
        }
    }
    true
}

/// Shared triangulation supports concave simple polygons, not just a convex
/// fan. Invalid/self-crossing polygons produce a diagnostic before rendering.
pub fn triangulate_polygon(points: &[[f32; 2]]) -> Result<Vec<[usize; 3]>, String> {
    if points.len() < 3 || points.len() > MAX_DRAW_POINTS || !simple_polygon(points) {
        return Err(
            "Canvas polygons must be simple, nondegenerate and contain 3–256 distinct points"
                .into(),
        );
    }
    let area: f64 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| f64::from(a[0]) * f64::from(b[1]) - f64::from(b[0]) * f64::from(a[1]))
        .sum();
    let sign = area.signum();
    let mut remaining: Vec<_> = (0..points.len()).collect();
    let mut triangles = Vec::new();
    while remaining.len() > 3 {
        let mut ear = None;
        for i in 0..remaining.len() {
            let a = remaining[(i + remaining.len() - 1) % remaining.len()];
            let b = remaining[i];
            let c = remaining[(i + 1) % remaining.len()];
            if cross(points[a], points[b], points[c]) * sign <= 1.0e-8 {
                continue;
            }
            let inside = remaining.iter().any(|p| {
                *p != a
                    && *p != b
                    && *p != c
                    && cross(points[a], points[b], points[*p]) * sign >= -1.0e-8
                    && cross(points[b], points[c], points[*p]) * sign >= -1.0e-8
                    && cross(points[c], points[a], points[*p]) * sign >= -1.0e-8
            });
            if !inside {
                ear = Some((i, [a, b, c]));
                break;
            }
        }
        let Some((index, triangle)) = ear else {
            return Err("Canvas polygon could not be triangulated safely".into());
        };
        triangles.push(triangle);
        remaining.remove(index);
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    Ok(triangles)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transformed_clips_and_hits_share_painter_geometry() {
        let drawing = CanvasDrawing {
            primitives: vec![CanvasPrimitive::Group {
                transform: [20.0, 30.0, 2.0, 2.0, 90.0, 0.5],
                clip: Some([0.0, 0.0, 10.0, 10.0]),
                children: vec![
                    CanvasPrimitive::Hit {
                        id: "first".into(),
                        rect: [0.0, 0.0, 20.0, 20.0],
                    },
                    CanvasPrimitive::Hit {
                        id: "last".into(),
                        rect: [0.0, 0.0, 10.0, 10.0],
                    },
                ],
            }],
        };
        let items = flatten(&drawing).unwrap();
        assert_eq!(items[0].opacity, 0.5);
        assert_eq!(hit_test(&items, [10.0, 40.0]), Some("last".into()));
        assert_eq!(hit_test(&items, [-10.0, 40.0]), None);
        assert!(inverse_point([0.0; 6], [0.0, 0.0]).is_none());
    }
    #[test]
    fn bounds_unknown_fields_and_resources_are_checked() {
        assert!(CanvasDrawing::parse(
            serde_json::json!([{"kind":"image","asset":"../secret","rect":[0,0,10,10]}])
        )
        .is_err());
        assert!(CanvasDrawing::parse(serde_json::json!([{"kind":"rect","rect":[0,0,10,10],"color":[1,0,0,1],"radius":0,"unknown":1}])).is_err());
        let primitive = CanvasPrimitive::Rect {
            rect: [0.0, 0.0, 1.0, 1.0],
            color: [1.0; 4],
            radius: 0.0,
        };
        assert!(CanvasDrawing {
            primitives: vec![primitive; 1025]
        }
        .validate()
        .is_err());
        assert!(CanvasDrawing::default().validate().is_ok());
    }
    #[test]
    fn concave_polygons_triangulate_and_self_intersections_fail() {
        let concave = [
            [0.0, 0.0],
            [10.0, 0.0],
            [5.0, 5.0],
            [10.0, 10.0],
            [0.0, 10.0],
        ];
        assert_eq!(triangulate_polygon(&concave).unwrap().len(), 3);
        assert!(
            triangulate_polygon(&[[0.0, 0.0], [10.0, 10.0], [0.0, 10.0], [10.0, 0.0]]).is_err()
        );
    }
}
