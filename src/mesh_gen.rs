//! Generative modifiers: Screw and Wave.
//!
//! Modelled on Blender:
//!  - `MOD_screw`: the profile (the mesh edges) is revolved about the object's Z axis by `angle` radians in
//!    `steps` steps, rising by `offset` over one full turn (the Screw offset). A full turn (360 degrees)
//!    closes the ring instead of duplicating the first one. The profile is the boundary edges of the mesh;
//!    a closed mesh has none, so every edge is revolved instead (Blender revolves all edges of a face mesh
//!    too, which is why a face profile is normally a loose edge strip). Blender's Flip option is not
//!    modelled: the quads follow the direction of the profile edges.
//!  - `MOD_wave`: a travelling sine ripple along Z. Both axes are enabled (circular wave starting at the
//!    origin): `z += height * sin(2 pi (d - speed * time) / (2 width))` where `d` is the distance from the
//!    origin in the XY plane. The Narrowness / Falloff / Lifetime options are not modelled.
//!
//! Screw changes the topology; Wave keeps the faces of its input.

use crate::mesh::{Mesh, V3};
use std::collections::HashMap;

pub const SCREW_STEPS: u32 = 16;

impl Mesh {
    /// The Screw modifier about the Z axis; see the module documentation. `offset` is the rise over a full turn.
    pub fn screwed(&self, angle: f32, steps: u32, offset: f32) -> Mesh {
        let steps = steps.max(1);
        let full = (angle.abs() - std::f32::consts::TAU).abs() < 1e-4;
        let rings = if full { steps } else { steps + 1 };
        /* Profile edges, each once, in the direction of the face that owns them. */
        let mut count: HashMap<(u32, u32), u32> = HashMap::new();
        let mut directed: Vec<(u32, u32)> = Vec::new();
        for f in &self.faces {
            for i in 0..f.len() {
                let (a, b) = (f[i], f[(i + 1) % f.len()]);
                let key = (a.min(b), a.max(b));
                let c = count.entry(key).or_insert(0);
                if *c == 0 {
                    directed.push((a, b));
                }
                *c += 1;
            }
        }
        let boundary: Vec<(u32, u32)> =
            directed.iter().copied().filter(|&(a, b)| count[&(a.min(b), a.max(b))] == 1).collect();
        let profile = if boundary.is_empty() { directed } else { boundary };
        if profile.is_empty() {
            return self.clone();
        }
        /* One ring of `rings` copies per used vertex. */
        let mut ring_of: HashMap<u32, u32> = HashMap::new();
        let mut out = Mesh::default();
        for &(a, b) in &profile {
            for v in [a, b] {
                if ring_of.contains_key(&v) {
                    continue;
                }
                ring_of.insert(v, out.verts.len() as u32);
                let p = self.verts[v as usize];
                for i in 0..rings {
                    let t = i as f32 / steps as f32;
                    let (s, c) = (angle * t).sin_cos();
                    let rise = offset * (angle / std::f32::consts::TAU) * t;
                    out.verts.push([p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2] + rise]);
                }
            }
        }
        for &(a, b) in &profile {
            let (ra, rb) = (ring_of[&a], ring_of[&b]);
            for i in 0..steps {
                let j = (i + 1) % rings;
                out.faces.push(vec![ra + i, rb + i, rb + j, ra + j]);
            }
        }
        /* Points on the axis collapse into one: weld them and drop the faces that vanish. */
        out.merged_by_distance(1e-5).0
    }

    /// The Wave modifier: a circular sine ripple along Z, see the module documentation.
    pub fn waved(&self, height: f32, width: f32, speed: f32, time: f32) -> Mesh {
        let w = width.max(1e-4);
        let verts: Vec<V3> = self
            .verts
            .iter()
            .map(|v| {
                let d = (v[0] * v[0] + v[1] * v[1]).sqrt();
                let phase = std::f32::consts::TAU * (d - speed * time) / (2.0 * w);
                [v[0], v[1], v[2] + height * phase.sin()]
            })
            .collect();
        Mesh { verts, faces: self.faces.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plane_spins_into_a_closed_ring() {
        /* The 4 boundary edges of a plane revolved in 16 steps: 4 vertices x 16, 4 x 16 quads. */
        let plane = Mesh::plane(2.0).mapped(|v| [v[0] + 3.0, v[1], v[2]], false);
        let s = plane.screwed(std::f32::consts::TAU, SCREW_STEPS, 0.0);
        assert_eq!(s.faces.len(), 4 * 16);
        assert_eq!(s.verts.len(), 4 * 16);
        assert!(s.edges().iter().all(|e| e.faces == 2), "a full turn is closed");
    }

    #[test]
    fn a_partial_turn_is_open_and_the_offset_lifts_it() {
        let plane = Mesh::plane(2.0).mapped(|v| [v[0] + 3.0, v[1], v[2]], false);
        let s = plane.screwed(std::f32::consts::PI, 8, 0.0);
        assert_eq!(s.verts.len(), 4 * 9);
        let up = plane.screwed(std::f32::consts::TAU, 8, 4.0);
        let top = up.verts.iter().map(|v| v[2]).fold(f32::MIN, f32::max);
        let base = plane.verts.iter().map(|v| v[2]).fold(f32::MIN, f32::max);
        assert!(top > base + 3.0 && top < base + 4.0, "7/8 of the rise is reached before the ring closes");
    }

    #[test]
    fn wave_keeps_the_topology_and_ripples_z() {
        let g = Mesh::grid(8, 8, 4.0);
        let w = g.waved(0.1, 1.0, 0.25, 0.0);
        assert_eq!(w.faces, g.faces);
        assert_ne!(w.verts, g.verts);
        assert!(w.verts.iter().all(|v| v[2].abs() <= 0.1 + 1e-5));
        assert_eq!(g.waved(0.0, 1.0, 0.25, 0.0), g);
        /* The x and y of every vertex are untouched. */
        assert!(g.verts.iter().zip(&w.verts).all(|(a, b)| a[0] == b[0] && a[1] == b[1]));
    }
}
