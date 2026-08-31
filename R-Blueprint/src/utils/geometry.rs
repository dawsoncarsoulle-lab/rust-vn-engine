//! Geometry utilities for blueprint rendering

use glam::{Vec2, Vec3, Mat2, Mat3, Mat4};
use std::f32::consts::*;;

/// Calculate the distance between two points
pub fn distance(p1: Vec2, p2: Vec2) -> f32 {
    (p2 - p1).length()
}

/// Calculate the squared distance between two points (faster, no sqrt)
pub fn distance_squared(p1: Vec2, p2: Vec2) -> f32 {
    (p2 - p1).length_squared()
}

/// Check if a point is inside a rectangle
pub fn point_in_rect(point: Vec2, min: Vec2, max: Vec2) -> bool {
    point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
}

/// Check if a point is inside a circle
pub fn point_in_circle(point: Vec2, center: Vec2, radius: f32) -> bool {
    distance_squared(point, center) <= radius * radius
}

/// Line-line intersection
/// Returns the intersection point if lines intersect, or None if they don't
pub fn line_line_intersection(
    p1: Vec2,
    p2: Vec2,
    p3: Vec2,
    p4: Vec2,
) -> Option<Vec2> {
    let denom = (p4.y - p3.y) * (p2.x - p1.x) - (p4.x - p3.x) * (p2.y - p1.y);
    
    if denom.abs() < 1e-6 {
        return None; // Lines are parallel
    }
    
    let ua = ((p4.x - p3.x) * (p1.y - p3.y) - (p4.y - p3.y) * (p1.x - p3.x)) / denom;
    let ub = ((p2.x - p1.x) * (p1.y - p3.y) - (p2.y - p1.y) * (p1.x - p3.x)) / denom;
    
    if ua >= 0.0 && ua <= 1.0 && ub >= 0.0 && ub <= 1.0 {
        Some(p1 + (p2 - p1) * ua)
    } else {
        None
    }
}

/// Point-line distance
pub fn point_line_distance(point: Vec2, line_start: Vec2, line_end: Vec2) -> f32 {
    let line_vec = line_end - line_start;
    let point_vec = point - line_start;
    
    let line_length = line_vec.length();
    if line_length < 1e-6 {
        return distance(point, line_start);
    }
    
    let projection = point_vec.dot(line_vec) / line_length;
    let closest_point = if projection <= 0.0 {
        line_start
    } else if projection >= line_length {
        line_end
    } else {
        line_start + line_vec * (projection / line_length)
    };
    
    distance(point, closest_point)
}

/// Project a point onto a line
pub fn project_point_onto_line(point: Vec2, line_start: Vec2, line_end: Vec2) -> Vec2 {
    let line_vec = line_end - line_start;
    let point_vec = point - line_start;
    
    let line_length = line_vec.length();
    if line_length < 1e-6 {
        return line_start;
    }
    
    let projection = point_vec.dot(line_vec) / line_length;
    
    if projection <= 0.0 {
        line_start
    } else if projection >= line_length {
        line_end
    } else {
        line_start + line_vec * (projection / line_length)
    }
}

/// Calculate the closest point on a line segment to a given point
pub fn closest_point_on_line_segment(point: Vec2, line_start: Vec2, line_end: Vec2) -> Vec2 {
    project_point_onto_line(point, line_start, line_end)
}

/// Check if two line segments intersect
pub fn line_segment_intersection(
    p1: Vec2,
    p2: Vec2,
    p3: Vec2,
    p4: Vec2,
) -> bool {
    line_line_intersection(p1, p2, p3, p4).is_some()
}

/// Calculate the angle between two vectors in radians
pub fn angle_between(v1: Vec2, v2: Vec2) -> f32 {
    v1.normalize().dot(v2.normalize()).acos()
}

/// Calculate the angle between two vectors in degrees
pub fn angle_between_degrees(v1: Vec2, v2: Vec2) -> f32 {
    angle_between(v1, v2).to_degrees()
}

/// Rotate a point around another point
pub fn rotate_point_around(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let cos = angle.cos();
    let sin = angle.sin();
    
    let translated = point - center;
    let rotated = Vec2::new(
        translated.x * cos - translated.y * sin,
        translated.x * sin + translated.y * cos,
    );
    
    rotated + center
}

/// Calculate the normal of a vector (perpendicular)
pub fn normal(v: Vec2) -> Vec2 {
    Vec2::new(-v.y, v.x).normalize()
}

/// Calculate the left normal of a vector
pub fn left_normal(v: Vec2) -> Vec2 {
    Vec2::new(-v.y, v.x).normalize()
}

/// Calculate the right normal of a vector
pub fn right_normal(v: Vec2) -> Vec2 {
    Vec2::new(v.y, -v.x).normalize()
}

/// Linear interpolation between two values
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation between two points
pub fn lerp_vec2(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    a + (b - a) * t
}

/// Smoothstep interpolation
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Calculate bezier curve point
pub fn cubic_bezier(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let one_minus_t = 1.0 - t;
    
    p0 * (one_minus_t * one_minus_t * one_minus_t) +
    p1 * (3.0 * one_minus_t * one_minus_t * t) +
    p2 * (3.0 * one_minus_t * t * t) +
    p3 * (t * t * t)
}

/// Calculate quadratic bezier curve point
pub fn quadratic_bezier(p0: Vec2, p1: Vec2, p2: Vec2, t: f32) -> Vec2 {
    let one_minus_t = 1.0 - t;
    
    p0 * (one_minus_t * one_minus_t) +
    p1 * (2.0 * one_minus_t * t) +
    p2 * (t * t)
}

/// Calculate the tangent of a cubic bezier curve at point t
pub fn cubic_bezier_tangent(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let one_minus_t = 1.0 - t;
    
    let derivative = 
        p1 * 3.0 * one_minus_t * one_minus_t +
        p2 * 6.0 * one_minus_t * t +
        p3 * 3.0 * t * t -
        p0 * 3.0 * one_minus_t * one_minus_t;
    
    derivative.normalize()
}

/// Calculate the length of a bezier curve
pub fn bezier_curve_length(
    p0: Vec2,
    p1: Vec2,
    p2: Vec2,
    p3: Vec2,
    steps: usize,
) -> f32 {
    let mut length = 0.0;
    let mut prev_point = p0;
    
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let point = cubic_bezier(p0, p1, p2, p3, t);
        length += distance(prev_point, point);
        prev_point = point;
    }
    
    length
}

/// Calculate points along a bezier curve
pub fn sample_bezier_curve(
    p0: Vec2,
    p1: Vec2,
    p2: Vec2,
    p3: Vec2,
    steps: usize,
) -> Vec<Vec2> {
    let mut points = Vec::with_capacity(steps + 1);
    
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        points.push(cubic_bezier(p0, p1, p2, p3, t));
    }
    
    points
}

/// Calculate the bounding box of a set of points
pub fn bounding_box(points: &[Vec2]) -> (Vec2, Vec2) {
    if points.is_empty() {
        return (Vec2::ZERO, Vec2::ZERO);
    }
    
    let mut min = points[0];
    let mut max = points[0];
    
    for &point in points.iter().skip(1) {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
    }
    
    (min, max)
}

/// Expand a bounding box by a margin
pub fn expand_bounding_box(min: Vec2, max: Vec2, margin: f32) -> (Vec2, Vec2) {
    (
        Vec2::new(min.x - margin, min.y - margin),
        Vec2::new(max.x + margin, max.y + margin),
    )
}

/// Check if a bounding box contains a point
pub fn bounding_box_contains_point(min: Vec2, max: Vec2, point: Vec2) -> bool {
    point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
}

/// Check if two bounding boxes intersect
pub fn bounding_boxes_intersect(
    min1: Vec2,
    max1: Vec2,
    min2: Vec2,
    max2: Vec2,
) -> bool {
    min1.x <= max2.x && max1.x >= min2.x &&
    min1.y <= max2.y && max1.y >= min2.y
}

/// Calculate the center of a bounding box
pub fn bounding_box_center(min: Vec2, max: Vec2) -> Vec2 {
    (min + max) / 2.0
}

/// Calculate the size of a bounding box
pub fn bounding_box_size(min: Vec2, max: Vec2) -> Vec2 {
    max - min
}

/// Clamp a value between min and max
pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max)
}

/// Clamp a point to a rectangle
pub fn clamp_point_to_rect(point: Vec2, min: Vec2, max: Vec2) -> Vec2 {
    Vec2::new(
        clamp(point.x, min.x, max.x),
        clamp(point.y, min.y, max.y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_distance() {
        assert!((distance(Vec2::ZERO, Vec2::new(3.0, 4.0)) - 5.0).abs() < 1e-6);
    }
    
    #[test]
    fn test_point_in_rect() {
        assert!(point_in_rect(Vec2::new(1.0, 1.0), Vec2::ZERO, Vec2::new(2.0, 2.0)));
        assert!(!point_in_rect(Vec2::new(3.0, 3.0), Vec2::ZERO, Vec2::new(2.0, 2.0)));
    }
    
    #[test]
    fn test_point_in_circle() {
        assert!(point_in_circle(Vec2::ZERO, Vec2::ZERO, 1.0));
        assert!(!point_in_circle(Vec2::new(2.0, 0.0), Vec2::ZERO, 1.0));
    }
    
    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-6);
    }
    
    #[test]
    fn test_cubic_bezier() {
        // Test that bezier passes through start and end points
        let p0 = Vec2::new(0.0, 0.0);
        let p1 = Vec2::new(1.0, 1.0);
        let p2 = Vec2::new(2.0, -1.0);
        let p3 = Vec2::new(3.0, 0.0);
        
        assert!((cubic_bezier(p0, p1, p2, p3, 0.0) - p0).length() < 1e-6);
        assert!((cubic_bezier(p0, p1, p2, p3, 1.0) - p3).length() < 1e-6);
    }
}
