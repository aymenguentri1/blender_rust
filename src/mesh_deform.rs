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
//!  - Bend (added to Simple Deform): the Z range is wrapped onto a circular arc of `amount` radians in the
//!    XZ plane, the lower end stays put
//!  - `MOD_displace`: every vertex moves along its normal by `(texture - midlevel) * strength`; the texture is
//!    a smooth value noise in 0..1 (Blender's default Clouds texture is also noise based), so coincident
//!    vertices move together and the mesh stays closed
//!
//!  - `MOD_cast`: vertices blend by `factor` towards a sphere (all axes) or a cylinder (X and Y only) around
//!    the object origin; the radius is the average distance of the vertices when it is 0
//!
//! None of them change the topology, so faces are shared with the input mesh.

use crate::mesh::{Mesh, V3};

/// The Displace modifier's Direction: along the vertex normals (the default) or along a local axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplaceDir {
    Normal,
    X,
    Y,
    Z,
}

impl DisplaceDir {
    pub fn label(self) -> &'static str {
        match self {
            DisplaceDir::Normal => "Normal",
            DisplaceDir::X => "X",
            DisplaceDir::Y => "Y",
            DisplaceDir::Z => "Z",
        }
    }

    /// Blender's menu order: Normal, X, Y, Z, then round again.
    pub fn next(self) -> DisplaceDir {
        match self {
            DisplaceDir::Normal => DisplaceDir::X,
            DisplaceDir::X => DisplaceDir::Y,
            DisplaceDir::Y => DisplaceDir::Z,
            DisplaceDir::Z => DisplaceDir::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeformMode {
    Twist,
    Bend,
    Taper,
    Stretch,
}

impl DeformMode {
    pub fn label(self) -> &'static str {
        match self {
            DeformMode::Twist => "Twist",
            DeformMode::Bend => "Bend",
            DeformMode::Taper => "Taper",
            DeformMode::Stretch => "Stretch",
        }
    }
    /// The next mode in the cycle, or `None` after the last one (the modifier is then removed).
    pub fn next(self) -> Option<DeformMode> {
        match self {
            DeformMode::Twist => Some(DeformMode::Bend),
            DeformMode::Bend => Some(DeformMode::Taper),
            DeformMode::Taper => Some(DeformMode::Stretch),
            DeformMode::Stretch => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastShape {
    Sphere,
    Cylinder,
}

impl CastShape {
    pub fn label(self) -> &'static str {
        match self {
            CastShape::Sphere => "Sphere",
            CastShape::Cylinder => "Cylinder",
        }
    }
    /// The next shape in the cycle, or `None` after the last one (the modifier is then removed).
    pub fn next(self) -> Option<CastShape> {
        match self {
            CastShape::Sphere => Some(CastShape::Cylinder),
            CastShape::Cylinder => None,
        }
    }
}

impl Mesh {
    /// The Cast modifier. `radius` 0 means the average distance of the vertices (measured in the cast plane).
    pub fn casted(&self, shape: CastShape, factor: f32, radius: f32) -> Mesh {
        let plane = |v: V3| -> V3 {
            match shape {
                CastShape::Sphere => v,
                CastShape::Cylinder => [v[0], v[1], 0.0],
            }
        };
        let len = |v: V3| plane(v).iter().map(|c| c * c).sum::<f32>().sqrt();
        let r = if radius > 0.0 {
            radius
        } else if self.verts.is_empty() {
            0.0
        } else {
            self.verts.iter().map(|&v| len(v)).sum::<f32>() / self.verts.len() as f32
        };
        let verts = self
            .verts
            .iter()
            .map(|&v| {
                let l = len(v);
                if l < 1e-9 {
                    return v;
                }
                let k = r / l;
                let target = match shape {
                    CastShape::Sphere => [v[0] * k, v[1] * k, v[2] * k],
                    CastShape::Cylinder => [v[0] * k, v[1] * k, v[2]],
                };
                [
                    v[0] + (target[0] - v[0]) * factor,
                    v[1] + (target[1] - v[1]) * factor,
                    v[2] + (target[2] - v[2]) * factor,
                ]
            })
            .collect();
        Mesh { verts, faces: self.faces.clone() }
    }

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

    /// The Displace modifier: moves each vertex along its normal by `(noise - midlevel) * strength`.
    #[cfg(test)]
    pub fn displaced(&self, strength: f32, midlevel: f32, noise_scale: f32) -> Mesh {
        self.displaced_along(strength, midlevel, noise_scale, DisplaceDir::Normal)
    }

    /// `displaced()` along `dir`: the vertex normals, or the local X, Y or Z axis.
    pub fn displaced_along(&self, strength: f32, midlevel: f32, noise_scale: f32, dir: DisplaceDir) -> Mesh {
        let normals = match dir {
            DisplaceDir::Normal => self.vertex_normals(),
            DisplaceDir::X => vec![[1.0, 0.0, 0.0]; self.verts.len()],
            DisplaceDir::Y => vec![[0.0, 1.0, 0.0]; self.verts.len()],
            DisplaceDir::Z => vec![[0.0, 0.0, 1.0]; self.verts.len()],
        };
        let verts = self
            .verts
            .iter()
            .zip(&normals)
            .map(|(v, n)| {
                let t = value_noise([v[0] * noise_scale, v[1] * noise_scale, v[2] * noise_scale]);
                let d = (t - midlevel) * strength;
                [v[0] + n[0] * d, v[1] + n[1] * d, v[2] + n[2] * d]
            })
            .collect();
        Mesh { verts, faces: self.faces.clone() }
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
                DeformMode::Bend => {
                    if amount.abs() < 1e-6 {
                        return v;
                    }
                    let r = span / amount;
                    let phi = amount * t;
                    let arm = r - x;
                    [r - arm * phi.cos(), y, lower + arm * phi.sin()]
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

fn lattice_hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77) ^ (z as u32).wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Smooth value noise in 0..1 (trilinear with a smoothstep fade between hashed lattice values).
pub fn value_noise(p: V3) -> f32 {
    let (fx, fy, fz) = (p[0].floor(), p[1].floor(), p[2].floor());
    let (ix, iy, iz) = (fx as i32, fy as i32, fz as i32);
    let fade = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v, w) = (fade(p[0] - fx), fade(p[1] - fy), fade(p[2] - fz));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c = |dx, dy, dz| lattice_hash(ix + dx, iy + dy, iz + dz);
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), u);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), u);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), u);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), u);
    lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displace_along_an_axis_moves_only_that_axis() {
        let cube = Mesh::cube(2.0);
        for (dir, axis) in [(DisplaceDir::X, 0), (DisplaceDir::Y, 1), (DisplaceDir::Z, 2)] {
            let d = cube.displaced_along(0.5, 0.0, 1.0, dir);
            assert_ne!(d.verts, cube.verts);
            for (a, b) in d.verts.iter().zip(&cube.verts) {
                for k in 0..3 {
                    if k != axis {
                        assert_eq!(a[k], b[k], "{dir:?} moved axis {k}");
                    }
                }
            }
        }
        assert_eq!(cube.displaced_along(0.5, 0.5, 1.0, DisplaceDir::Normal), cube.displaced(0.5, 0.5, 1.0));
        assert_eq!(DisplaceDir::Z.next().next(), DisplaceDir::X);
    }

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

    #[test]
    fn bend_keeps_the_bottom_and_curls_the_top() {
        let cube = Mesh::cube(2.0);
        let b = cube.deformed(DeformMode::Bend, std::f32::consts::FRAC_PI_2);
        let r = 2.0 / std::f32::consts::FRAC_PI_2;
        for (a, n) in cube.verts.iter().zip(&b.verts) {
            if a[2] < 0.0 {
                assert!((a[0] - n[0]).abs() < 1e-5 && (a[2] - n[2]).abs() < 1e-5);
            } else if a[0].abs() < 1.0001 && a[0] > 0.0 {
                /* x = 1 is the inner side: radius r - 1, a quarter turn lands it at x = r, z = -1 + r - 1. */
                assert!((n[0] - r).abs() < 1e-4, "{n:?}");
                assert!((n[2] - (-1.0 + r - 1.0)).abs() < 1e-4, "{n:?}");
            }
            assert_eq!(a[1], n[1]);
        }
        assert_eq!(cube.deformed(DeformMode::Bend, 0.0), cube);
    }

    #[test]
    fn displace_is_deterministic_and_keeps_the_mesh_closed() {
        let cube = Mesh::cube(2.0);
        let d = cube.displaced(0.5, 0.5, 1.0);
        assert_eq!(d, cube.displaced(0.5, 0.5, 1.0));
        assert_ne!(d.verts, cube.verts);
        assert_eq!(d.faces, cube.faces);
        assert_eq!(cube.displaced(0.0, 0.5, 1.0), cube);
        let n = value_noise([0.3, 1.7, -2.2]);
        assert!((0.0..=1.0).contains(&n));
    }

    #[test]
    fn full_cast_puts_every_vertex_on_the_sphere_or_cylinder() {
        let cube = Mesh::cube(2.0);
        let s = cube.casted(CastShape::Sphere, 1.0, 0.0);
        for v in &s.verts {
            let l = v.iter().map(|c| c * c).sum::<f32>().sqrt();
            assert!((l - 3.0f32.sqrt()).abs() < 1e-4);
        }
        let c = cube.casted(CastShape::Cylinder, 1.0, 2.0);
        for (a, v) in cube.verts.iter().zip(&c.verts) {
            assert!(((v[0] * v[0] + v[1] * v[1]).sqrt() - 2.0).abs() < 1e-4);
            assert_eq!(a[2], v[2]);
        }
        assert_eq!(cube.casted(CastShape::Sphere, 0.0, 1.0), cube);
    }
}
