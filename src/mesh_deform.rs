//! Vertex-position modifiers: Smooth and Simple Deform.
//!
//! Modelled on Blender:
//!  - `MOD_smooth`: every iteration moves a vertex towards the average of its edge neighbours by `factor`
//!    (defaults: factor 0.5, 1 repeat); vertices without edges stay put
//!  - `MOD_simpledeform` (deform axis Z): the mesh is measured along the axis, positions are clamped to
//!    that range and the effect grows linearly from the lower end (0) to the upper end (1) of it:
//!    Twist rotates the cross-section about the axis by up to `amount` radians, Taper scales it by
//!    `1 + amount * t` and Stretch bulges it by `amount * sin(pi t)` (a smooth, ends-fixed stretch)
//!
//! None of them change the topology, so faces are shared with the input mesh.

use crate::mesh::{Mesh, V3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeformMode {
    Twist,
    Taper,
    Stretch,
}

impl DeformMode {
    pub fn label(self) -> &'static str {
        match self {
            DeformMode::Twist => "Twist",
            DeformMode::Taper => "Taper",
            DeformMode::Stretch => "Stretch",
        }
    }
    /// The next mode in the cycle, or `None` after the last one (the modifier is then removed).
    pub fn next(self) -> Option<DeformMode> {
        match self {
            DeformMode::Twist => Some(DeformMode::Taper),
            DeformMode::Taper => Some(DeformMode::Stretch),
            DeformMode::Stretch => None,
        }
    }
}

impl Mesh {
    /// The Smooth modifier: `repeat` rounds of moving each vertex `factor` of the way to its neighbour average.
    pub fn smoothed(&self, factor: f32, repeat: u32) -> Mesh {
        let n = self.verts.len();
        let mut neighbours: Vec<Vec<u32>> = vec![Vec::new(); n];
        for e in self.edges() {
            neighbours[e.a as usize].push(e.b);
            neighbours[e.b as usize].push(e.a);
        }
        let mut cur = self.verts.clone();
        for _ in 0..repeat {
            let mut next = cur.clone();
            for i in 0..n {
                let nb = &neighbours[i];
                if nb.is_empty() {
                    continue;
                }
                let mut avg = [0.0f32; 3];
                for &j in nb {
                    for k in 0..3 {
                        avg[k] += cur[j as usize][k];
                    }
                }
                let inv = 1.0 / nb.len() as f32;
                for k in 0..3 {
                    next[i][k] = cur[i][k] * (1.0 - factor) + avg[k] * inv * factor;
                }
            }
            cur = next;
        }
        Mesh { verts: cur, faces: self.faces.clone() }
    }

    /// The Simple Deform modifier about the Z axis. `amount` is in radians for Twist and a factor otherwise.
    pub fn deformed(&self, mode: DeformMode, amount: f32) -> Mesh {
        let Some((min, max)) = self.bounds() else {
            return self.clone();
        };
        let (lower, upper) = (min[2], max[2]);
        let span = upper - lower;
        if span < 1e-9 {
            return self.clone();
        }
        let map = |v: V3| -> V3 {
            let t = ((v[2].clamp(lower, upper)) - lower) / span;
            let (x, y) = (v[0], v[1]);
            match mode {
                DeformMode::Twist => {
                    let (s, c) = (amount * t).sin_cos();
                    [x * c - y * s, x * s + y * c, v[2]]
                }
                DeformMode::Taper => {
                    let k = 1.0 + amount * t;
                    [x * k, y * k, v[2]]
                }
                DeformMode::Stretch => {
                    let k = 1.0 + amount * (std::f32::consts::PI * t).sin();
                    [x * k, y * k, v[2]]
                }
            }
        };
        Mesh { verts: self.verts.iter().map(|&v| map(v)).collect(), faces: self.faces.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothing_shrinks_a_cube_towards_its_center() {
        let cube = Mesh::cube(2.0);
        let s = cube.smoothed(0.5, 1);
        assert_eq!(s.faces, cube.faces);
        let r0 = cube.verts[0].iter().map(|c| c * c).sum::<f32>();
        let r1 = s.verts[0].iter().map(|c| c * c).sum::<f32>();
        assert!(r1 < r0);
        assert_eq!(cube.smoothed(0.0, 3), cube);
        assert_eq!(cube.smoothed(0.5, 0), cube);
    }

    #[test]
    fn twist_keeps_the_bottom_and_rotates_the_top() {
        let cube = Mesh::cube(2.0);
        let t = cube.deformed(DeformMode::Twist, std::f32::consts::FRAC_PI_2);
        for (a, b) in cube.verts.iter().zip(&t.verts) {
            if a[2] < 0.0 {
                assert!((a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5);
            } else {
                /* A quarter turn: (x, y) -> (-y, x). */
                assert!((b[0] + a[1]).abs() < 1e-5 && (b[1] - a[0]).abs() < 1e-5);
            }
            assert_eq!(a[2], b[2]);
        }
    }

    #[test]
    fn taper_scales_the_top_and_stretch_fixes_both_ends() {
        let cube = Mesh::cube(2.0);
        let t = cube.deformed(DeformMode::Taper, -0.5);
        let top = t.verts.iter().zip(&cube.verts).find(|(_, a)| a[2] > 0.0).unwrap();
        assert!((top.0[0] - top.1[0] * 0.5).abs() < 1e-5);
        let s = cube.deformed(DeformMode::Stretch, 1.0);
        for (a, b) in cube.verts.iter().zip(&s.verts) {
            assert!((a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4);
        }
    }
}
