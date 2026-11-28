//! Geometry Nodes "Spiral" (nodes/geometry/nodes/node_geo_curve_primitive_spiral.cc): `int(resolution *
//! rotations) + 1` points whose radius and height grow linearly; the `Reverse` input flips the turning direction
//! (true = counterclockwise, false = clockwise, as Blender's `direction` argument). Deviation: positions only.

#![allow(dead_code)]

use std::f32::consts::PI;

type V3 = [f32; 3];

/// `create_spiral_curve()`.
pub fn create_spiral(rotations: f32, resolution: i32, start_radius: f32, end_radius: f32, height: f32, direction: bool) -> Vec<V3> {
    let total = ((resolution as f32 * rotations) as i32).max(1);
    let n = total as f32;
    let delta_radius = (end_radius - start_radius) / n;
    let delta_height = height / n;
    let delta_theta = (PI * 2.0 * rotations) / n * if direction { 1.0 } else { -1.0 };
    (0..=total)
        .map(|i| {
            let f = i as f32;
            let theta = f * delta_theta;
            let radius = start_radius + f * delta_radius;
            [radius * theta.cos(), radius * theta.sin(), delta_height * f]
        })
        .collect()
}

/// `node_geo_exec()`: rotations clamp to >= 0 (0 gives no curve), resolution to >= 1.
pub fn spiral(rotations: f32, resolution: i32, start_radius: f32, end_radius: f32, height: f32, reverse: bool) -> Option<Vec<V3>> {
    let rotations = rotations.max(0.0);
    if rotations == 0.0 {
        return None;
    }
    Some(create_spiral(rotations, resolution.max(1), start_radius, end_radius, height, reverse))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_count_start_end_radius_and_height() {
        let s = spiral(2.0, 32, 1.0, 2.0, 2.0, true).unwrap();
        assert_eq!(s.len(), 65);
        assert_eq!(s[0], [1.0, 0.0, 0.0]);
        let e = s[64];
        assert!((e[0] - 2.0).abs() < 1e-4 && e[1].abs() < 1e-4 && (e[2] - 2.0).abs() < 1e-5);
        let r = |p: V3| (p[0] * p[0] + p[1] * p[1]).sqrt();
        assert!((r(s[32]) - 1.5).abs() < 1e-4);
    }

    #[test]
    fn direction_flag_mirrors_y() {
        let a = create_spiral(1.0, 8, 1.0, 1.0, 0.0, true);
        let b = create_spiral(1.0, 8, 1.0, 1.0, 0.0, false);
        assert!(a[2][1] > 0.0 && b[2][1] < 0.0);
        assert!((a[2][0] - b[2][0]).abs() < 1e-6 && (a[2][1] + b[2][1]).abs() < 1e-6);
    }

    #[test]
    fn zero_rotations_gives_none_and_small_counts_clamp() {
        assert!(spiral(0.0, 32, 1.0, 2.0, 1.0, true).is_none());
        assert!(spiral(-1.0, 32, 1.0, 2.0, 1.0, true).is_none());
        assert_eq!(spiral(0.01, 4, 1.0, 1.0, 1.0, true).unwrap().len(), 2); // total clamped to 1
        assert_eq!(spiral(1.0, 0, 1.0, 1.0, 1.0, true).unwrap().len(), 2); // resolution clamped to 1
    }
}
