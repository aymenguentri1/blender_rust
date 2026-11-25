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
//!  - `MOD_laplaciansmooth`: Laplacian smoothing (`factor` per round, `repeat` rounds) followed, like Blender's
//!    default Preserve Volume option, by a uniform scale about the centroid that restores the volume of a
//!    closed mesh. Uniform edge weights stand in for Blender's cotangent weights.
//!  - `MOD_remesh` in Blocks mode: the mesh is voxelised at `voxel` m (a vertical ray per column finds the
//!    inside cells; the grid is capped at 64 cells along its longest side) and the faces between a filled and
//!    an empty cell become quads. It needs a closed mesh; an open one has no inside and gives an empty mesh.
//!    Blender's Smooth, Sharp and Voxel modes are not modelled.
//!
//!  - `MOD_shrinkwrap`, Nearest Surface Point: every vertex moves onto the closest point of the target's
//!    triangles, then `offset` metres away from the surface towards the side the vertex came from (the
//!    triangle normal when it was already on the surface). The target is the base mesh of the other object
//!    (its own modifiers are ignored). Blender's other wrap methods, the wrap mode and the vertex group are
//!    not modelled; the search is brute force, so keep the target below a few thousand triangles.
//!
//! Screw and Remesh change the topology; Wave and Laplacian Smooth keep the faces of their input.

use crate::mesh::{add, cross, dot, normalized, scale, sub, Mesh, V3};
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

    /// Laplacian Smooth with Preserve Volume, see the module documentation.
    pub fn laplacian_smoothed(&self, factor: f32, repeat: u32) -> Mesh {
        let mut out = self.smoothed(factor, repeat);
        let (v0, v1) = (self.signed_volume(), out.signed_volume());
        if v0 > 1e-9 && v1 > 1e-9 && !out.verts.is_empty() {
            let k = (v0 / v1).cbrt();
            let n = out.verts.len() as f32;
            let c = out.verts.iter().fold([0.0f32; 3], |a, v| [a[0] + v[0] / n, a[1] + v[1] / n, a[2] + v[2] / n]);
            for v in out.verts.iter_mut() {
                *v = [c[0] + (v[0] - c[0]) * k, c[1] + (v[1] - c[1]) * k, c[2] + (v[2] - c[2]) * k];
            }
        }
        out
    }

    /// The Remesh modifier in Blocks mode, see the module documentation.
    pub fn remeshed_blocks(&self, voxel: f32) -> Mesh {
        let Some((mn, mx)) = self.bounds() else { return Mesh::default() };
        let size = [mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]];
        let longest = size[0].max(size[1]).max(size[2]);
        let h = voxel.max(longest / 64.0).max(1e-4);
        /* One cell of padding on each side keeps the outer faces inside the grid. */
        let origin = [mn[0] - h, mn[1] - h, mn[2] - h];
        let dim: [usize; 3] = std::array::from_fn(|k| ((size[k] / h).ceil() as usize + 2).max(1));
        let idx = |x: usize, y: usize, z: usize| (z * dim[1] + y) * dim[0] + x;
        let mut filled = vec![false; dim[0] * dim[1] * dim[2]];
        let tris: Vec<[V3; 3]> = self
            .triangles()
            .into_iter()
            .map(|(_, t)| [self.verts[t[0] as usize], self.verts[t[1] as usize], self.verts[t[2] as usize]])
            .collect();
        for y in 0..dim[1] {
            for x in 0..dim[0] {
                /* An irrational-ish nudge keeps the ray off exact vertices and edges. */
                let (px, py) = (origin[0] + (x as f32 + 0.5) * h + 1.7e-5 * h, origin[1] + (y as f32 + 0.5) * h + 2.3e-5 * h);
                let mut hits: Vec<f32> = Vec::new();
                for t in &tris {
                    let d = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]);
                    if d.abs() < 1e-12 {
                        continue;
                    }
                    let l1 = ((px - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (py - t[0][1])) / d;
                    let l2 = ((t[1][0] - t[0][0]) * (py - t[0][1]) - (px - t[0][0]) * (t[1][1] - t[0][1])) / d;
                    if l1 >= 0.0 && l2 >= 0.0 && l1 + l2 <= 1.0 {
                        hits.push(t[0][2] + l1 * (t[1][2] - t[0][2]) + l2 * (t[2][2] - t[0][2]));
                    }
                }
                hits.sort_by(|a, b| a.partial_cmp(b).unwrap());
                for pair in hits.chunks_exact(2) {
                    for z in 0..dim[2] {
                        let zc = origin[2] + (z as f32 + 0.5) * h;
                        if zc > pair[0] && zc < pair[1] {
                            filled[idx(x, y, z)] = true;
                        }
                    }
                }
            }
        }
        let at = |x: i64, y: i64, z: i64| {
            x >= 0 && y >= 0 && z >= 0 && (x as usize) < dim[0] && (y as usize) < dim[1] && (z as usize) < dim[2]
                && filled[idx(x as usize, y as usize, z as usize)]
        };
        let mut corner: HashMap<(usize, usize, usize), u32> = HashMap::new();
        let mut out = Mesh::default();
        let mut vert = |c: (usize, usize, usize), out: &mut Mesh| -> u32 {
            *corner.entry(c).or_insert_with(|| {
                out.verts.push([origin[0] + c.0 as f32 * h, origin[1] + c.1 as f32 * h, origin[2] + c.2 as f32 * h]);
                (out.verts.len() - 1) as u32
            })
        };
        /* (axis, direction, the four corners as offsets, counter-clockwise seen from outside). */
        for z in 0..dim[2] {
            for y in 0..dim[1] {
                for x in 0..dim[0] {
                    if !filled[idx(x, y, z)] {
                        continue;
                    }
                    let (xi, yi, zi) = (x as i64, y as i64, z as i64);
                    let faces: [([i64; 3], [[usize; 3]; 4]); 6] = [
                        ([1, 0, 0], [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]]),
                        ([-1, 0, 0], [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]]),
                        ([0, 1, 0], [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]]),
                        ([0, -1, 0], [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]]),
                        ([0, 0, 1], [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]]),
                        ([0, 0, -1], [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]]),
                    ];
                    for (n, quad) in faces {
                        if at(xi + n[0], yi + n[1], zi + n[2]) {
                            continue;
                        }
                        let f: Vec<u32> = quad.iter().map(|o| vert((x + o[0], y + o[1], z + o[2]), &mut out)).collect();
                        out.faces.push(f);
                    }
                }
            }
        }
        out
    }

    /// The Shrinkwrap modifier (Nearest Surface Point); `target` is in the space of `self`.
    pub fn shrinkwrapped(&self, target: &Mesh, offset: f32) -> Mesh {
        let tris: Vec<[V3; 3]> = target
            .triangles()
            .into_iter()
            .map(|(_, t)| [target.verts[t[0] as usize], target.verts[t[1] as usize], target.verts[t[2] as usize]])
            .collect();
        if tris.is_empty() {
            return self.clone();
        }
        let verts = self
            .verts
            .iter()
            .map(|&p| {
                let mut best = (f32::MAX, p, 0usize);
                for (i, t) in tris.iter().enumerate() {
                    let q = closest_on_triangle(p, t);
                    let d = sub(p, q);
                    let d2 = dot(d, d);
                    if d2 < best.0 {
                        best = (d2, q, i);
                    }
                }
                let (d2, q, i) = best;
                if offset == 0.0 {
                    return q;
                }
                let t = &tris[i];
                let n = normalized(cross(sub(t[1], t[0]), sub(t[2], t[0])));
                let dir = if d2 > 1e-12 { normalized(sub(p, q)) } else { n };
                add(q, scale(dir, offset))
            })
            .collect();
        Mesh { verts, faces: self.faces.clone() }
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

/// The closest point of triangle `t` to `p` (Ericson, Real-Time Collision Detection 5.1.5).
pub(crate) fn closest_on_triangle(p: V3, t: &[V3; 3]) -> V3 {
    let (a, b, c) = (t[0], t[1], t[2]);
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return add(a, scale(ab, d1 / (d1 - d3)));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return add(a, scale(ac, d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return add(b, scale(sub(c, b), w));
    }
    let denom = 1.0 / (va + vb + vc);
    add(a, add(scale(ab, vb * denom), scale(ac, vc * denom)))
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
    fn laplacian_smooth_keeps_the_volume() {
        let sphere = Mesh::uv_sphere(16, 8, 1.0);
        let noisy = sphere.displaced(0.3, 0.5, 3.0);
        let s = noisy.laplacian_smoothed(0.5, 3);
        assert_eq!(s.faces, noisy.faces);
        assert_ne!(s.verts, noisy.verts);
        assert!((s.signed_volume() - noisy.signed_volume()).abs() < 1e-3 * noisy.signed_volume().abs());
    }

    #[test]
    fn remesh_blocks_fills_a_cube_with_cells() {
        let cube = Mesh::cube(2.0);
        let r = cube.remeshed_blocks(0.5);
        /* 4 x 4 x 4 cells: 6 sides of 16 quads, closed and outward facing, the cube's volume. */
        assert_eq!(r.faces.len(), 6 * 16);
        assert!(r.edges().iter().all(|e| e.faces == 2));
        assert!((r.signed_volume() - 8.0).abs() < 1e-3);
        /* A plane is open: nothing is inside. */
        assert!(Mesh::plane(2.0).remeshed_blocks(0.5).faces.is_empty());
    }

    #[test]
    fn shrinkwrap_snaps_onto_the_target_surface() {
        /* A cube's vertices pulled onto a bigger sphere end up on the sphere's surface (within the facets). */
        let sphere = Mesh::uv_sphere(32, 16, 2.0);
        let cube = Mesh::cube(2.0);
        let w = cube.shrinkwrapped(&sphere, 0.0);
        assert_eq!(w.faces, cube.faces);
        for v in &w.verts {
            let r = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            assert!((r - 2.0).abs() < 0.1, "{r}");
        }
        /* The offset keeps the side the vertex came from: a big cube's corners land outside the sphere. */
        let off = Mesh::cube(8.0).shrinkwrapped(&sphere, 0.5);
        assert!(off.verts.iter().all(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() > 2.3));
        /* An empty target changes nothing. */
        assert_eq!(cube.shrinkwrapped(&Mesh::default(), 0.0), cube);
    }

    #[test]
    fn closest_point_handles_face_edge_and_corner() {
        let t = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let f = closest_on_triangle([0.2, 0.2, 3.0], &t);
        assert!((f[0] - 0.2).abs() < 1e-6 && (f[1] - 0.2).abs() < 1e-6 && f[2] == 0.0);
        assert_eq!(closest_on_triangle([2.0, -1.0, 0.0], &t), [1.0, 0.0, 0.0]);
        assert_eq!(closest_on_triangle([-1.0, -1.0, 0.0], &t), [0.0, 0.0, 0.0]);
        let e = closest_on_triangle([1.0, 1.0, 0.0], &t);
        assert!((e[0] - 0.5).abs() < 1e-6 && (e[1] - 0.5).abs() < 1e-6);
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
