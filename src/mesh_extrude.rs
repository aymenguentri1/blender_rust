//! Edge and vertex extrude of an edit-mesh selection (E).
//!
//! Ported from `bmesh/operators/bmo_extrude.cc`:
//!  - `bmo_extrude_edge_only_exec()` (165): the selected edges and their vertices are duplicated (a shared vertex is
//!    duplicated once); every original edge gets a quad `(v1, v2, new v2, new v1)` when the first face that uses the
//!    edge runs `v2 -> v1` over it (`edge_normal_flip == use_normal_flip`), else the reversed quad, so the new face
//!    winds against its neighbour; a wire edge (no face) takes the reversed branch. The new edges stay selected.
//!  - `bmo_extrude_vert_indiv_exec()` (234): each vertex is duplicated and joined to its copy by a new edge; `Mesh`
//!    has no loose edges, so that edge is returned as a pair next to the mesh (the wire is not drawn).
//!    Not ported: select history, skin-root flags, loop attribute copy (no custom data in `Mesh`).

use crate::mesh::Mesh;
use std::collections::HashMap;

/// Result of an extrude: the new mesh and what the operator selects afterwards (`geom.out`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extruded {
    pub mesh: Mesh,
    /// Duplicated vertices, in order of first appearance in the input.
    pub new_verts: Vec<u32>,
    /// Edges that stay selected: the duplicates of the input edges (`EXT_KEEP`).
    pub new_edges: Vec<(u32, u32)>,
    /// Faces created between each input edge and its duplicate.
    pub new_faces: Vec<u32>,
    /// Face extrude only: the copies of the input faces (`geom.out` faces).
    pub dup_faces: Vec<u32>,
}

/// Whether the first face that uses edge `(a, b)` runs `b -> a` over it (`e->l->v != e->v1`); `None` for a wire edge.
fn first_face_runs_backwards(mesh: &Mesh, a: u32, b: u32) -> Option<bool> {
    for f in &mesh.faces {
        let n = f.len();
        for i in 0..n {
            let (p, q) = (f[i], f[(i + 1) % n]);
            if p == a && q == b {
                return Some(false);
            }
            if p == b && q == a {
                return Some(true);
            }
        }
    }
    None
}

/// `bmo_extrude_edge_only_exec()`: extrudes `edges` (vertex index pairs, duplicates and degenerate pairs ignored).
/// `use_normal_flip` is the operator's `use_normal_flip` (false from `MESH_OT_extrude_edges_move`).
pub fn extrude_edges(mesh: &Mesh, edges: &[(u32, u32)], use_normal_flip: bool) -> Extruded {
    let n = mesh.verts.len() as u32;
    let mut out = Extruded { mesh: mesh.clone(), ..Default::default() };
    let mut dup: HashMap<u32, u32> = HashMap::new();
    let mut seen: Vec<(u32, u32)> = Vec::new();
    for &(a, b) in edges {
        let key = (a.min(b), a.max(b));
        if a == b || a >= n || b >= n || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        for v in [a, b] {
            dup.entry(v).or_insert_with(|| {
                out.mesh.verts.push(mesh.verts[v as usize]);
                out.new_verts.push(out.mesh.verts.len() as u32 - 1);
                out.mesh.verts.len() as u32 - 1
            });
        }
        let (na, nb) = (dup[&a], dup[&b]);
        let normal_flip = match first_face_runs_backwards(mesh, a, b) {
            Some(backwards) => !backwards,
            None => true,
        };
        let quad = if normal_flip == use_normal_flip { vec![a, b, nb, na] } else { vec![b, a, na, nb] };
        out.mesh.faces.push(quad);
        out.new_faces.push(out.mesh.faces.len() as u32 - 1);
        out.new_edges.push((na, nb));
    }
    out
}

/// `bmo_extrude_vert_indiv_exec()`: duplicates every vertex in `verts` (repeats ignored); the wire edge from each vertex
/// to its copy is in `new_edges`.
pub fn extrude_verts(mesh: &Mesh, verts: &[u32]) -> Extruded {
    let n = mesh.verts.len() as u32;
    let mut out = Extruded { mesh: mesh.clone(), ..Default::default() };
    let mut seen: Vec<u32> = Vec::new();
    for &v in verts {
        if v >= n || seen.contains(&v) {
            continue;
        }
        seen.push(v);
        out.mesh.verts.push(mesh.verts[v as usize]);
        let nv = out.mesh.verts.len() as u32 - 1;
        out.new_verts.push(nv);
        out.new_edges.push((v, nv));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn up(m: &Mesh, f: usize) -> f32 {
        m.face_normal(f)[2]
    }

    #[test]
    fn extruding_a_plane_boundary_edge_adds_a_quad_with_matching_winding() {
        let plane = Mesh::plane(2.0);
        let e = plane.edges().into_iter().find(|e| e.faces == 1).unwrap();
        let r = extrude_edges(&plane, &[(e.a, e.b)], false);
        assert_eq!((r.mesh.verts.len(), r.mesh.faces.len()), (6, 2));
        assert_eq!((r.new_verts.len(), r.new_edges.len(), r.new_faces.len()), (2, 1, 1));
        assert_eq!(r.mesh.verts[4], plane.verts[e.a as usize]);
        assert!(up(&r.mesh, 0) > 0.99);
        assert!(!r.mesh.edges().iter().any(|x| x.faces > 2), "non manifold");
        /* The new quad shares the edge with the plane and traverses it the other way round. */
        let q = &r.mesh.faces[1];
        let n = q.len();
        let runs = |f: &[u32], a: u32, b: u32| (0..f.len()).any(|i| f[i] == a && f[(i + 1) % f.len()] == b);
        let plane_runs_ab = runs(&plane.faces[0], e.a, e.b);
        assert_eq!(runs(q, e.a, e.b), !plane_runs_ab);
        assert_eq!(n, 4);
        assert!(up(&r.mesh, 1).abs() < 1e-6, "the copy sits on the original, so the quad is flat and has no normal yet");
    }

    #[test]
    fn use_normal_flip_reverses_the_new_face() {
        let plane = Mesh::plane(2.0);
        let e = plane.edges().into_iter().find(|e| e.faces == 1).unwrap();
        let a = extrude_edges(&plane, &[(e.a, e.b)], false);
        let b = extrude_edges(&plane, &[(e.a, e.b)], true);
        let mut rev = a.mesh.faces[1].clone();
        rev.reverse();
        let mut got = b.mesh.faces[1].clone();
        let k = got.iter().position(|&x| x == rev[0]).unwrap();
        got.rotate_left(k);
        assert_eq!(got, rev);
    }

    #[test]
    fn two_connected_edges_share_one_duplicated_vertex() {
        let plane = Mesh::plane(2.0);
        let (a, b) = (plane.faces[0][0], plane.faces[0][1]);
        let c = plane.faces[0][2];
        let r = extrude_edges(&plane, &[(a, b), (b, c), (b, a), (7, 7), (0, 99)], false);
        assert_eq!((r.new_verts.len(), r.new_faces.len(), r.new_edges.len()), (3, 2, 2));
        assert_eq!(r.mesh.verts.len(), 7);
        assert_eq!(r.new_edges[0].1, r.new_edges[1].0, "the shared vertex has one copy");
    }

    #[test]
    fn a_wire_edge_takes_the_reversed_branch() {
        let m = Mesh { verts: vec![[0.0; 3], [1.0, 0.0, 0.0]], faces: vec![] };
        let r = extrude_edges(&m, &[(0, 1)], false);
        assert_eq!(r.mesh.faces, vec![vec![1, 0, 2, 3]]);
    }

    #[test]
    fn vertex_extrude_duplicates_in_place_and_returns_the_wire() {
        let plane = Mesh::plane(2.0);
        let r = extrude_verts(&plane, &[2, 0, 2, 50]);
        assert_eq!(r.mesh.verts.len(), 6);
        assert_eq!(r.mesh.faces, plane.faces);
        assert_eq!(r.new_edges, vec![(2, 4), (0, 5)]);
        assert_eq!(r.mesh.verts[4], plane.verts[2]);
    }
}
