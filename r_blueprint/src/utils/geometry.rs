//! Geometry utilities for blueprint rendering

use glam::Vec2;
use std::f32::consts::*;

/// Calculate the distance between two points
pub fn distance(p1: Vec2, p2: Vec2) -> f32 {
    (p2 - p1).length()
}

/// Calculate the midpoint between two points
pub fn midpoint(p1: Vec2, p2: Vec2) -> Vec2 {
    (p1 + p2) / 2.0
}

/// Line-line intersection
/// Returns the intersection point if lines intersect, None otherwise
pub fn line_line_intersection(
    p1: Vec2,
    p2: Vec2,
    p3: Vec2,
    p4: Vec2,
) -> Option<Vec2> {
    let denominator = (p4.y - p3.y) * (p2.x - p1.x) - (p4.x - p3.x) * (p2.y - p1.y);
    
    if denominator.abs() < 1e-6 {
        return None;
    }
    
    let ua = ((p4.x - p3.x) * (p1.y - p3.y) - (p4.y - p3.y) * (p1.x - p3.x)) / denominator;
    let ub = ((p2.x - p1.x) * (p1.y - p3.y) - (p2.y - p1.y) * (p1.x - p3.x)) / denominator;
    
    if ua >= 0.0 && ua <= 1.0 && ub >= 0.0 && ub <= 1.0 {
        let x = p1.x + ua * (p2.x - p1.x);
        let y = p1.y + ua * (p2.y - p1.y);
        Some(Vec2::new(x, y))
    } else {
        None
    }
}

/// Cubic bezier curve calculation
pub fn cubic_bezier(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;
    
    p0 * mt3 + p1 * 3.0 * mt2 * t + p2 * 3.0 * mt * t2 + p3 * t3
}

/// Quadratic bezier curve calculation
pub fn quadratic_bezier(p0: Vec2, p1: Vec2, p2: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    
    p0 * mt2 + p1 * 2.0 * mt * t + p2 * t2
}

/// Calculate bounding box for a set of points
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

/// Check if a point is inside a rectangle
pub fn point_in_rect(point: Vec2, min: Vec2, max: Vec2) -> bool {
    point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
}

/// Calculate the length of a bezier curve
pub fn bezier_curve_length(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, segments: usize) -> f32 {
    let mut length = 0.0;
    let mut prev_point = p0;
    
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let point = cubic_bezier(p0, p1, p2, p3, t);
        length += (point - prev_point).length();
        prev_point = point;
    }
    
    length
}

/// Get a point on a bezier curve at a specific distance
pub fn bezier_point_at_distance(
    p0: Vec2,
    p1: Vec2,
    p2: Vec2,
    p3: Vec2,
    distance: f32,
    segments: usize,
) -> Vec2 {
    let total_length = bezier_curve_length(p0, p1, p2, p3, segments);
    let target_ratio = (distance / total_length).clamp(0.0, 1.0);
    
    // Binary search for the point at the target distance
    let mut low = 0.0;
    let mut high = 1.0;
    
    for _ in 0..10 {
        let mid = (low + high) / 2.0;
        let mid_length = bezier_curve_length(p0, p1, p2, p3, (mid * segments as f32) as usize);
        
        if mid_length < distance {
            low = mid;
        } else {
            high = mid;
        }
    }
    
    cubic_bezier(p0, p1, p2, p3, (low + high) / 2.0)
}

/// Calculate the angle between two vectors in degrees
pub fn angle_between(v1: Vec2, v2: Vec2) -> f32 {
    let dot = v1.dot(v2);
    let det = v1.x * v2.y - v1.y * v2.x;
    det.atan2(dot).to_degrees()
}

/// Rotate a point around another point
pub fn rotate_point(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let s = angle.to_radians().sin();
    let c = angle.to_radians().cos();
    
    // Translate point back to origin
    let px = point.x - center.x;
    let py = point.y - center.y;
    
    // Rotate point
    let xnew = px * c - py * s;
    let ynew = px * s + py * c;
    
    // Translate point back
    Vec2::new(xnew + center.x, ynew + center.y)
}

/// Clamp a value between min and max
pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max)
}

/// Linear interpolation
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Vector2 linear interpolation
pub fn lerp_vec2(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    Vec2::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t))
}

/// Smoothstep function for smooth interpolation
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
