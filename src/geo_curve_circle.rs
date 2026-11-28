//! Geometry Nodes "Curve Circle" (nodes/geometry/nodes/node_geo_curve_primitive_circle.cc): a cyclic poly curve
//! from a radius (XY plane) or through three points (also returns the center). Resolution is clamped to >= 3.
//! Deviation: returns positions only; cyclic flag and Curves container are the caller's job.

#![allow(dead_code)]

use std::f32::consts::PI;

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn normalize(v: V3) -> V3 {
    let l = dot(v, v).sqrt();
    if l == 0.0 { [0.0; 3] } else { [v[0] / l, v[1] / l, v[2] / l] }
}

/// `create_radius_circle_curve()`: `resolution` points `(r cos t, r sin t, 0)`, `t = 2pi i / resolution`.
pub fn radius_circle(resolution: i32, radius: f32) -> Vec<V3> {
    let res = resolution.max(3);
    let step = (2.0 * PI) / res as f32;
    (0..res).map(|i| { let t = step * i as f32; [radius * t.cos(), radius * t.sin(), 0.0] }).collect()
}

/// `colinear_f3_f3_f3()`: exact comparison of the normalized directions, as in Blender.
fn colinear(p1: V3, p2: V3, p3: V3) -> bool {
    let a = normalize(sub(p2, p1));
    let b = normalize(sub(p3, p1));
    a == b || a == [-b[0], -b[1], -b[2]]
}

/// `isect_plane_plane_plane_v3()`: intersection of planes `n_i . x = d_i`, None when (nearly) parallel.
fn isect_three_planes(n: [V3; 3], d: [f32; 3]) -> Option<V3> {
    let det = dot(n[0], cross(n[1], n[2]));
    if det.abs() <= f32::EPSILON {
        return None;
    }
    let (c12, c20, c01) = (cross(n[1], n[2]), cross(n[2], n[0]), cross(n[0], n[1]));
    Some(std::array::from_fn(|i| (d[0] * c12[i] + d[1] * c20[i] + d[2] * c01[i]) / det))
}

/// `create_point_circle_curve()`: circle through three points. `(positions, center)`; `None` and a zero center
/// when the points are colinear or the planes do not meet in one point.
pub fn point_circle(p1: V3, p2: V3, p3: V3, resolution: i32) -> (Option<Vec<V3>>, V3) {
    let res = resolution.max(3);
    if colinear(p1, p2, p3) {
        return (None, [0.0; 3]);
    }
    let q1: V3 = std::array::from_fn(|i| p1[i] + (p2[i] - p1[i]) * 0.5);
    let q2: V3 = std::array::from_fn(|i| p2[i] + (p3[i] - p2[i]) * 0.5);
    let v1 = normalize(sub(p2, p1));
    let v2 = normalize(sub(p3, p2));
    let v3 = normalize(cross(v1, v2));
    let v4 = normalize(cross(v3, v1));
    let Some(center) = isect_three_planes([v3, v1, v2], [dot(v3, q1), dot(v1, q1), dot(v2, q2)]) else {
        return (None, [0.0; 3]);
    };
    let d = sub(p1, center);
    let r = dot(d, d).sqrt();
    let step = (2.0 * PI) / res as f32;
    let pts = (0..res)
        .map(|i| {
            let t = step * i as f32;
            std::array::from_fn(|k| center[k] + r * t.sin() * v1[k] + r * t.cos() * v4[k])
        })
        .collect();
    (Some(pts), center)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist(a: V3, b: V3) -> f32 { dot(sub(a, b), sub(a, b)).sqrt() }

    #[test]
    fn radius_circle_points_lie_on_radius_starting_on_x() {
        let p = radius_circle(8, 2.0);
        assert_eq!(p.len(), 8);
        assert_eq!(p[0], [2.0, 0.0, 0.0]);
        assert!(p.iter().all(|q| (dist(*q, [0.0; 3]) - 2.0).abs() < 1e-5 && q[2] == 0.0));
        assert!((p[2][1] - 2.0).abs() < 1e-5); // quarter turn
    }

    #[test]
    fn resolution_is_clamped_to_three() {
        assert_eq!(radius_circle(0, 1.0).len(), 3);
        assert_eq!(radius_circle(-5, 1.0).len(), 3);
    }

    #[test]
    fn three_points_recover_center_and_radius() {
        let (pts, c) = point_circle([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], 16);
        let pts = pts.unwrap();
        assert!(dist(c, [0.0; 3]) < 1e-5);
        assert!(pts.iter().all(|q| (dist(*q, c) - 1.0).abs() < 1e-4));
        assert!(pts.iter().all(|q| q[2].abs() < 1e-5));
        assert!(dist(pts[0], [1.0, 0.0, 0.0]) < 1e-5 || pts.iter().any(|q| dist(*q, [1.0, 0.0, 0.0]) < 1e-4));
    }

    #[test]
    fn offset_circle_center() {
        let (pts, c) = point_circle([3.0, 1.0, 5.0], [2.0, 2.0, 5.0], [1.0, 1.0, 5.0], 8);
        assert!(pts.is_some());
        assert!(dist(c, [2.0, 1.0, 5.0]) < 1e-4);
    }

    #[test]
    fn colinear_points_give_nothing() {
        let (pts, c) = point_circle([0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], 8);
        assert!(pts.is_none());
        assert_eq!(c, [0.0; 3]);
    }
}
