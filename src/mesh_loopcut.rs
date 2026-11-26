//! Loop cut: insert `cuts` edge loops across the edge ring of an edge (`mesh.loopcut_slide`, `editmesh_loopcut.cc`).
//!
//! Ported from `bmesh/operators/bmo_subdivide.cc` `bmo_subdivide_edges_exec()` as `edbm_ringsel_finish()` calls it:
//! the ring edges (`mesh_edgering`) are split at `i / (cuts + 1)` (`subdivide_edge_num`, no smoothness / falloff), then
//! each face is filled by the pattern of its split edges:
//!  - quad, two opposite split edges: `use_grid_fill` strips, `cuts + 1` quads between the new vertices;
//!  - quad, one split edge (`quad_1edge_split`, one cut): the cut vertex joins both far corners, three triangles;
//!  - triangle, one split edge (`tri_1edge_split`): the cut vertices join the apex, a triangle fan;
//!  - anything else (n-gons, a quad with `cuts > 1` and one split edge) keeps its face and only gains the new vertices.
//! Not ported: smoothness, fractal, `SUBD_FALLOFF_*`, the `quad_1edge` pattern for more than one cut, `custom_patterns`.

use crate::mesh::{Mesh, V3};
use crate::mesh_edgering::edge_ring;
use std::collections::HashMap;

type Edge = (u32, u32);

/// Result of [`loop_cut`]: the cut mesh and the new edges between the inserted vertices (what `ELE_INNER` selects).
#[derive(Debug, PartialEq)]
pub struct LoopCut {
    pub mesh: Mesh,
    pub inner_edges: Vec<Edge>,
}

fn key(a: u32, b: u32) -> Edge {
    (a.min(b), a.max(b))
}

fn lerp(a: V3, b: V3, t: f32) -> V3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Cut `cuts` loops across the ring of edge `(a, b)`. `None` when `cuts == 0` or the edge is not in the mesh.
pub fn loop_cut(mesh: &Mesh, a: u32, b: u32, cuts: usize) -> Option<LoopCut> {
    let ring = edge_ring(mesh, a, b);
    if cuts == 0 || ring.is_empty() {
        return None;
    }
    let mut out = mesh.clone();
    /* New vertices of each ring edge, ordered from the lower to the higher vertex index. */
    let mut split: HashMap<Edge, Vec<u32>> = HashMap::new();
    for &(lo, hi) in &ring {
        let (p, q) = (mesh.verts[lo as usize], mesh.verts[hi as usize]);
        let ids = (1..=cuts)
            .map(|i| {
                out.verts.push(lerp(p, q, i as f32 / (cuts + 1) as f32));
                (out.verts.len() - 1) as u32
            })
            .collect();
        split.insert(key(lo, hi), ids);
    }
    let mut inner = Vec::new();
    out.faces.clear();
    for face in &mesh.faces {
        let n = face.len();
        /* Corner `k` edge's new vertices in face direction, and the corners that have any. */
        let side = |k: usize| -> Vec<u32> {
            let (u, v) = (face[k], face[(k + 1) % n]);
            match split.get(&key(u, v)) {
                Some(ids) if u <= v => ids.clone(),
                Some(ids) => ids.iter().rev().copied().collect(),
                None => Vec::new(),
            }
        };
        let hit: Vec<usize> = (0..n).filter(|&k| !side(k).is_empty()).collect();
        match (n, hit.as_slice()) {
            (4, &[k, m]) if m == (k + 2) % 4 || k == (m + 2) % 4 => {
                let (k, m) = if m == (k + 2) % 4 { (k, m) } else { (m, k) };
                let (bot, top) = (side(k), side(m));
                let (f0, f1, f2, f3) = (face[k], face[(k + 1) % 4], face[m], face[(m + 1) % 4]);
                /* Bottom runs f0 -> f1, top runs f3 -> f2 (reversed along the face). */
                let b_pts: Vec<u32> = std::iter::once(f0).chain(bot.iter().copied()).chain([f1]).collect();
                let t_pts: Vec<u32> = std::iter::once(f3).chain(top.iter().rev().copied()).chain([f2]).collect();
                for i in 0..=cuts {
                    out.faces.push(vec![b_pts[i], b_pts[i + 1], t_pts[i + 1], t_pts[i]]);
                }
                for i in 0..cuts {
                    inner.push(key(bot[i], top[cuts - 1 - i]));
                }
            }
            (4, &[k]) if cuts == 1 => {
                let c = side(k)[0];
                let (v1, v2, v3, v0) = (face[(k + 1) % 4], face[(k + 2) % 4], face[(k + 3) % 4], face[k]);
                out.faces.extend([vec![c, v1, v2], vec![c, v2, v3], vec![c, v3, v0]]);
                inner.extend([key(c, v2), key(c, v3)]);
            }
            (3, &[k]) => {
                let ids = side(k);
                let (p, q, apex) = (face[k], face[(k + 1) % 3], face[(k + 2) % 3]);
                let pts: Vec<u32> = std::iter::once(p).chain(ids.iter().copied()).chain([q]).collect();
                for w in pts.windows(2) {
                    out.faces.push(vec![w[0], w[1], apex]);
                }
                inner.extend(ids.iter().map(|&c| key(c, apex)));
            }
            _ => {
                /* No pattern: the face keeps its shape and takes the new vertices along its split edges. */
                let mut f = Vec::new();
                for k in 0..n {
                    f.push(face[k]);
                    f.extend(side(k));
                }
                out.faces.push(f);
            }
        }
    }
    inner.sort();
    inner.dedup();
    Some(LoopCut { mesh: out, inner_edges: inner })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_cut_adds_a_loop_of_four_vertices_and_four_faces() {
        let m = Mesh::cube(2.0);
        let e = m.edges()[0];
        let r = loop_cut(&m, e.a, e.b, 1).unwrap();
        assert_eq!(r.mesh.verts.len(), 12);
        assert_eq!(r.mesh.faces.len(), 10);
        assert!(r.mesh.faces.iter().all(|f| f.len() == 4));
        assert_eq!(r.inner_edges.len(), 4);
    }

    #[test]
    fn cut_vertices_sit_at_the_fractions_of_the_edge() {
        let m = Mesh::cube(2.0);
        let e = m.edges()[0];
        let r = loop_cut(&m, e.a, e.b, 3).unwrap();
        let (p, q) = (m.verts[e.a as usize], m.verts[e.b as usize]);
        let new: Vec<V3> = r.mesh.verts[8..].to_vec();
        assert_eq!(new.len(), 12);
        for t in [0.25_f32, 0.5, 0.75] {
            let w = lerp(p, q, t);
            assert!(new.iter().any(|v| (0..3).all(|i| (v[i] - w[i]).abs() < 1e-5)), "{t}");
        }
        assert_eq!(r.mesh.faces.len(), 2 + 4 * 4);
    }

    fn total_area(m: &Mesh) -> f32 {
        let mut sum = 0.0;
        for f in &m.faces {
            for i in 1..f.len() - 1 {
                let (p, q, r) = (m.verts[f[0] as usize], m.verts[f[i] as usize], m.verts[f[i + 1] as usize]);
                let (u, v) = ([q[0] - p[0], q[1] - p[1]], [r[0] - p[0], r[1] - p[1]]);
                sum += (u[0] * v[1] - u[1] * v[0]) * 0.5;
            }
        }
        sum
    }

    #[test]
    fn grid_cut_keeps_area_and_winding() {
        let m = Mesh::grid(4, 4, 2.0);
        let e = m.edges().into_iter().find(|e| e.faces == 2).unwrap();
        let r = loop_cut(&m, e.a, e.b, 1).unwrap();
        assert_eq!(r.mesh.faces.len(), m.faces.len() + 4);
        assert!((total_area(&r.mesh) - total_area(&m)).abs() < 1e-4);
        for f in 0..r.mesh.faces.len() {
            assert!(r.mesh.face_normal(f)[2].abs() > 0.99, "face {f}");
            assert_eq!(r.mesh.face_normal(f)[2] > 0.0, m.face_normal(0)[2] > 0.0, "face {f} flipped");
        }
    }

    #[test]
    fn triangle_at_the_end_of_the_ring_becomes_two_triangles() {
        let m = Mesh { verts: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.5, -1.0, 0.0]], faces: vec![vec![0, 1, 2, 3], vec![1, 0, 4]] };
        let r = loop_cut(&m, 0, 1, 1).unwrap();
        assert_eq!(r.mesh.verts.len(), 7);
        assert_eq!(r.mesh.faces.len(), 4);
        assert_eq!(r.mesh.faces.iter().filter(|f| f.len() == 3).count(), 2);
    }

    #[test]
    fn ngon_at_the_end_of_the_ring_only_gains_the_vertex() {
        let m = Mesh { verts: vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.5, -1.0, 0.0], [2.0, -1.0, 0.0]], faces: vec![vec![0, 1, 2, 3], vec![1, 0, 4, 5, 2]] };
        let r = loop_cut(&m, 0, 1, 1).unwrap();
        assert_eq!(r.mesh.faces.len(), 3);
        assert_eq!(r.mesh.faces.iter().filter(|f| f.len() == 6).count(), 1);
    }

    #[test]
    fn zero_cuts_and_unknown_edge_are_none() {
        let m = Mesh::cube(2.0);
        assert!(loop_cut(&m, 0, 1, 0).is_none());
        assert!(loop_cut(&m, 20, 21, 1).is_none());
    }
}
