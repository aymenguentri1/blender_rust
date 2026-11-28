//! Geometry Nodes "Star" (nodes/geometry/nodes/node_geo_curve_primitive_star.cc): a cyclic poly curve alternating
//! points on an outer circle and an inner circle rotated by half a step plus `twist`. Radii clamp to >= 0 and
//! points to >= 3 as in `node_geo_exec`. Deviation: returns positions; the caller builds the Curves.

#![allow(dead_code)]

use std::f32::consts::PI;

type V3 = [f32; 3];

/// `create_star_curve()` (after the node's input clamping): `points * 2` positions, outer at even indices.
pub fn star(inner_radius: f32, outer_radius: f32, twist: f32, points: i32) -> Vec<V3> {
    let inner = inner_radius.max(0.0);
    let outer = outer_radius.max(0.0);
    let points = points.max(3);
    let step = (2.0 * PI) / points as f32;
    let mut out = Vec::with_capacity(points as usize * 2);
    for i in 0..points {
        let a = step * i as f32;
        out.push([outer * a.cos(), outer * a.sin(), 0.0]);
        let b = a + step * 0.5 + twist;
        out.push([inner * b.cos(), inner * b.sin(), 0.0]);
    }
    out
}

/// `create_selection_output()`: the "Outer Points" selection is true for even point indices.
pub fn outer_points_selection(point_count: usize) -> Vec<bool> {
    (0..point_count).map(|i| i % 2 == 0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(p: V3) -> f32 { (p[0] * p[0] + p[1] * p[1]).sqrt() }

    #[test]
    fn has_two_points_per_tip_with_alternating_radii() {
        let s = star(1.0, 2.0, 0.0, 5);
        assert_eq!(s.len(), 10);
        for (i, p) in s.iter().enumerate() {
            let want = if i % 2 == 0 { 2.0 } else { 1.0 };
            assert!((r(*p) - want).abs() < 1e-5 && p[2] == 0.0);
        }
        assert_eq!(s[0], [2.0, 0.0, 0.0]);
    }

    #[test]
    fn inner_points_sit_half_a_step_after_outer_and_twist_rotates_them() {
        let s = star(1.0, 2.0, 0.0, 4); // step = 90 deg, inner at 45 deg
        assert!((s[1][0] - s[1][1]).abs() < 1e-5 && s[1][0] > 0.0);
        let t = star(1.0, 2.0, PI / 4.0, 4); // inner now at 90 deg
        assert!(t[1][0].abs() < 1e-5 && (t[1][1] - 1.0).abs() < 1e-5);
        assert_eq!(s[0], t[0]); // outer points unaffected
    }

    #[test]
    fn inputs_are_clamped() {
        assert_eq!(star(1.0, 2.0, 0.0, 0).len(), 6);
        let s = star(-1.0, -2.0, 0.0, 3);
        assert!(s.iter().all(|p| p[0].abs() < 1e-6 && p[1].abs() < 1e-6));
        let big_inner = star(3.0, 1.0, 0.0, 3); // inner may exceed outer
        assert!((r(big_inner[1]) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn selection_marks_outer_points() {
        assert_eq!(outer_points_selection(5), vec![true, false, true, false, true]);
    }
}
