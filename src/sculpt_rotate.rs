//! Rotate brush maths (`mesh/brushes/rotate.cc` `calc_translations`, `do_rotate_brush`, and `BLI_dial_2d.cc`).
//!
//! A vertex under the brush turns around the area normal through the brush centre by `angle * falloff`, where
//! `angle` is the dial angle of the mouse around the stroke start (`vertex_rotation = -dial_angle * strength`).

// Wired into the stroke in the next step.
#![allow(dead_code)]

use crate::mesh::{add, cross, dot, normalized, scale, sub, V3};

/// `flip[mirror_symmetry_pass]` of `do_rotate_brush`; the pass is the bit mask of mirrored axes (X 1, Y 2, Z 4).
const FLIP: [f32; 8] = [1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0];

/// The sign of the angle for a mirror pass.
pub fn flip_sign(mirror_pass: usize) -> f32 {
    FLIP[mirror_pass & 7]
}

/// The mouse threshold of the Rotate brush dial (`pixel_input_threshold`).
pub const DIAL_THRESHOLD_PX: f32 = 5.0;

/// `Dial` of `BLI_dial_2d.cc`: the angle of the mouse around `center`, counted from the first direction past the threshold.
#[derive(Clone, Copy, Debug)]
pub struct Dial {
    center: [f32; 2],
    threshold_sq: f32,
    initial: [f32; 2],
    last_angle: f32,
    rotations: i32,
    initialized: bool,
}

impl Dial {
    pub fn new(start: [f32; 2], threshold: f32) -> Dial {
        Dial {
            center: start,
            threshold_sq: threshold * threshold,
            initial: [0.0; 2],
            last_angle: 0.0,
            rotations: 0,
            initialized: false,
        }
    }

    /// `BLI_dial_angle`.
    pub fn angle(&mut self, cur: [f32; 2]) -> f32 {
        let d = [cur[0] - self.center[0], cur[1] - self.center[1]];
        let len_sq = d[0] * d[0] + d[1] * d[1];
        if len_sq > self.threshold_sq {
            let len = len_sq.sqrt();
            let d = [d[0] / len, d[1] / len];
            if !self.initialized {
                self.initial = d;
                self.initialized = true;
            }
            let cosval = d[0] * self.initial[0] + d[1] * self.initial[1];
            let sinval = d[0] * self.initial[1] - d[1] * self.initial[0];
            let angle = sinval.atan2(cosval);
            if angle * self.last_angle < 0.0 && self.last_angle.abs() > std::f32::consts::FRAC_PI_2
            {
                if self.last_angle < 0.0 {
                    self.rotations -= 1;
                } else {
                    self.rotations += 1;
                }
            }
            self.last_angle = angle;
            return angle + 2.0 * std::f32::consts::PI * self.rotations as f32;
        }
        self.last_angle
    }
}

/// `vertex_rotation` of the stroke cache.
pub fn vertex_rotation(dial_angle: f32, strength: f32) -> f32 {
    -dial_angle * strength
}

/// `math::from_rotation<float3x3>(AxisAngle)` applied to `p - center` (Rodrigues), `axis` normalised here.
fn rotate_about(p: V3, axis: V3, angle: f32, center: V3) -> V3 {
    let k = normalized(axis);
    let v = sub(p, center);
    let (s, c) = angle.sin_cos();
    add(
        add(scale(v, c), scale(cross(k, v), s)),
        scale(k, dot(k, v) * (1.0 - c)),
    )
}

/// `calc_translations` for one vertex: the move that turns `pos` by `angle` around `axis` through `center`.
pub fn translation(pos: V3, axis: V3, angle: f32, center: V3) -> V3 {
    sub(add(rotate_about(pos, axis, angle, center), center), pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn len(v: V3) -> f32 {
        dot(v, v).sqrt()
    }

    #[test]
    fn full_angle_turns_a_vertex_about_the_normal() {
        let p = [1.0, 0.0, 0.5];
        let t = translation(p, [0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2, [0.0; 3]);
        let q = add(p, t);
        assert!((q[0]).abs() < 1e-5 && (q[1] - 1.0).abs() < 1e-5 && (q[2] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn zero_angle_is_identity() {
        let t = translation([0.3, -2.0, 1.0], [0.0, 1.0, 0.0], 0.0, [1.0, 1.0, 1.0]);
        assert!(len(t) < 1e-6);
    }

    #[test]
    fn tangent_radius_is_preserved() {
        let (c, n) = ([0.5, 0.5, 0.0], [0.0, 0.0, 2.0]);
        let p = [1.5, 0.9, 0.3];
        let q = add(p, translation(p, n, 1.1, c));
        let r = |v: V3| ((v[0] - c[0]).powi(2) + (v[1] - c[1]).powi(2)).sqrt();
        assert!((r(p) - r(q)).abs() < 1e-5 && (p[2] - q[2]).abs() < 1e-6);
    }

    #[test]
    fn dial_counts_from_the_first_direction_and_ignores_the_threshold() {
        let mut d = Dial::new([100.0, 100.0], DIAL_THRESHOLD_PX);
        assert_eq!(d.angle([102.0, 100.0]), 0.0);
        assert!(d.angle([150.0, 100.0]).abs() < 1e-6);
        let a = d.angle([100.0, 150.0]);
        assert!((a - -std::f32::consts::FRAC_PI_2).abs() < 1e-5, "{a}");
        assert!((vertex_rotation(a, 0.5) - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
    }

    #[test]
    fn dial_counts_full_turns() {
        let mut d = Dial::new([0.0, 0.0], 5.0);
        d.angle([10.0, 0.0]);
        let mut a = 0.0;
        for k in 1..=8 {
            let t = -(k as f32) * std::f32::consts::FRAC_PI_4;
            a = d.angle([10.0 * t.cos(), 10.0 * t.sin()]);
        }
        assert!((a - std::f32::consts::TAU).abs() < 1e-4, "{a}");
    }

    #[test]
    fn mirror_pass_flips_the_angle() {
        assert_eq!(
            [flip_sign(0), flip_sign(1), flip_sign(3), flip_sign(7)],
            [1.0, -1.0, 1.0, -1.0]
        );
    }
}
