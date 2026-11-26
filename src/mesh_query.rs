//! BMesh edge / vertex topology queries over `Mesh` (an edge is a sorted vertex pair, a loop is a face corner).
//!
//! Ported from `bmesh_query_inline.hh` (`BM_edge_is_wire`, `BM_edge_is_manifold`, `BM_edge_is_boundary`) and
//! `bmesh_query.cc` (`BM_vert_is_edge_pair`, `BM_vert_is_manifold`, with `BM_vert_step_fan_loop` and
//! `BM_edge_other_loop`). `Mesh` stores faces only, so an edge exists when a face uses it; "wire" (no face) is
//! therefore only true for an edge no face uses, as Blender's `e->l == nullptr`.

use crate::mesh::Mesh;
use std::collections::HashMap;

pub type Edge = (u32, u32);
/// A face corner: (face index, corner index). Its vertex is `faces[f][c]`, its edge runs to corner `c + 1`.
type Loop = (usize, usize);

fn key(a: u32, b: u32) -> Edge {
    (a.min(b), a.max(b))
}

/// Topology cache: the loops (face corners) on every edge, i.e. the radial cycle of each edge.
pub struct Topology<'a> {
    mesh: &'a Mesh,
    radial: HashMap<Edge, Vec<Loop>>,
}

impl<'a> Topology<'a> {
    pub fn new(mesh: &'a Mesh) -> Self {
        let mut radial: HashMap<Edge, Vec<Loop>> = HashMap::new();
        for (f, face) in mesh.faces.iter().enumerate() {
            for c in 0..face.len() {
                radial.entry(key(face[c], face[(c + 1) % face.len()])).or_default().push((f, c));
            }
        }
        Topology { mesh, radial }
    }

    fn face_count(&self, e: Edge) -> usize {
        self.radial.get(&key(e.0, e.1)).map_or(0, |l| l.len())
    }
    fn vert(&self, l: Loop) -> u32 {
        self.mesh.faces[l.0][l.1]
    }
    fn next(&self, l: Loop) -> Loop {
        (l.0, (l.1 + 1) % self.mesh.faces[l.0].len())
    }
    fn prev(&self, l: Loop) -> Loop {
        let n = self.mesh.faces[l.0].len();
        (l.0, (l.1 + n - 1) % n)
    }
    fn edge(&self, l: Loop) -> Edge {
        key(self.vert(l), self.vert(self.next(l)))
    }
    /// `l->radial_next`: next loop around the edge's radial cycle.
    fn radial_next(&self, l: Loop) -> Loop {
        let r = &self.radial[&self.edge(l)];
        let i = r.iter().position(|&x| x == l).unwrap();
        r[(i + 1) % r.len()]
    }

    /// `BM_edge_is_wire`: no face uses the edge.
    pub fn edge_is_wire(&self, e: Edge) -> bool {
        self.face_count(e) == 0
    }
    /// `BM_edge_is_manifold`: exactly two faces use the edge.
    pub fn edge_is_manifold(&self, e: Edge) -> bool {
        self.face_count(e) == 2
    }
    /// `BM_edge_is_boundary`: exactly one face uses the edge.
    pub fn edge_is_boundary(&self, e: Edge) -> bool {
        self.face_count(e) == 1
    }

    /// Distinct edges (as used by faces) that touch `v`.
    fn vert_edges(&self, v: u32) -> Vec<Edge> {
        let mut es: Vec<Edge> = self.radial.keys().copied().filter(|e| e.0 == v || e.1 == v).collect();
        es.sort_unstable();
        es
    }

    /// `BM_vert_is_edge_pair`: the vertex has exactly two edges.
    pub fn vert_is_edge_pair(&self, v: u32) -> bool {
        self.vert_edges(v).len() == 2
    }

    /// `BM_edge_other_loop`: loop on `e` (the other face) that starts at the same vertex as `l`.
    fn edge_other_loop(&self, e: Edge, l: Loop) -> Option<Loop> {
        let base = if self.edge(l) == e { l } else { self.prev(l) };
        let lo = self.radial_next(base);
        if self.vert(lo) == self.vert(l) {
            Some(lo)
        } else if self.vert(self.next(lo)) == self.vert(l) {
            Some(self.next(lo))
        } else {
            None
        }
    }

    /// `BM_vert_step_fan_loop`.
    fn step_fan_loop(&self, l: Loop, e_prev: &mut Edge) -> Option<Loop> {
        let e_next = if self.edge(l) == *e_prev {
            self.edge(self.prev(l))
        } else if self.edge(self.prev(l)) == *e_prev {
            self.edge(l)
        } else {
            return None;
        };
        if self.edge_is_manifold(e_next) {
            *e_prev = e_next;
            self.edge_other_loop(e_next, l)
        } else {
            None
        }
    }

    /// `BM_face_find_double`: walks the radial cycle of the face's first loop edge and returns the first face (other
    /// loop) of equal length whose edge sequence matches in the same or the opposite winding.
    pub fn face_find_double(&self, f: usize) -> Option<usize> {
        let l_first: Loop = (f, 0);
        let len = self.mesh.faces[f].len();
        let mut l_iter = self.radial_next(l_first);
        while l_iter != l_first {
            if self.mesh.faces[l_iter.0].len() == len {
                let same = self.vert(l_iter) == self.vert(l_first);
                let (mut l_a, mut l_b) = (l_first, l_iter);
                let mut matched = true;
                loop {
                    if self.edge(l_a) != self.edge(l_b) {
                        matched = false;
                        break;
                    }
                    l_a = if same { self.next(l_a) } else { self.prev(l_a) };
                    l_b = self.next(l_b);
                    if l_b == l_iter {
                        break;
                    }
                }
                if matched {
                    return Some(l_iter.0);
                }
            }
            l_iter = self.radial_next(l_iter);
        }
        None
    }

    /// `BM_vert_is_manifold`: the faces around the vertex form a single fan (open or closed).
    pub fn vert_is_manifold(&self, v: u32) -> bool {
        let edges = self.vert_edges(v);
        if edges.is_empty() {
            return false; // loose vertex
        }
        let (mut loop_num, mut boundary_num) = (0, 0);
        let mut l_first = self.radial[&edges[0]][0];
        for &e in &edges {
            let n = self.face_count(e);
            if n == 0 || n > 2 {
                return false; // wire edge or more than two faces
            }
            let l = self.radial[&e][0];
            if self.vert(l) == v {
                loop_num += 1;
            }
            if !self.edge_is_boundary(e) {
                if self.vert(self.radial_next(l)) == v {
                    loop_num += 1;
                }
            } else {
                l_first = l;
                boundary_num += 1;
                if boundary_num == 3 {
                    return false;
                }
            }
        }
        let mut e_prev = self.edge(l_first);
        if self.vert(l_first) != v {
            l_first = self.next(l_first);
        }
        let mut l_iter = l_first;
        let mut region = 0;
        loop {
            region += 1;
            match self.step_fan_loop(l_iter, &mut e_prev) {
                Some(n) if n != l_first => l_iter = n,
                _ => break,
            }
            if region > 4 * self.mesh.faces.len() + 4 {
                return false;
            }
        }
        loop_num == region
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(verts: usize, faces: &[&[u32]]) -> Mesh {
        Mesh { verts: vec![[0.0; 3]; verts], faces: faces.iter().map(|f| f.to_vec()).collect() }
    }

    #[test]
    fn cube_is_all_manifold() {
        let mesh = Mesh::cube(2.0);
        let t = Topology::new(&mesh);
        for e in t.radial.keys() {
            assert!(t.edge_is_manifold(*e) && !t.edge_is_boundary(*e) && !t.edge_is_wire(*e));
        }
        for v in 0..8 {
            assert!(t.vert_is_manifold(v));
            assert!(!t.vert_is_edge_pair(v)); // three edges each
        }
    }

    #[test]
    fn open_plane_boundary() {
        let mesh = m(4, &[&[0, 1, 2, 3]]);
        let t = Topology::new(&mesh);
        assert!(t.edge_is_boundary((1, 0)) && !t.edge_is_manifold((0, 1)));
        assert!(t.edge_is_wire((0, 2))); // no face uses it
        for v in 0..4 {
            assert!(t.vert_is_edge_pair(v));
            assert!(t.vert_is_manifold(v)); // single open fan
        }
    }

    #[test]
    fn non_manifold_fan() {
        // three triangles sharing edge (0,1): non-manifold edge and vertices
        let mesh = m(5, &[&[0, 1, 2], &[1, 0, 3], &[0, 1, 4]]);
        let t = Topology::new(&mesh);
        assert!(!t.edge_is_manifold((0, 1)) && !t.edge_is_boundary((0, 1)));
        assert!(!t.vert_is_manifold(0) && !t.vert_is_manifold(1));
        assert!(t.vert_is_manifold(2)); // lone corner of one triangle
    }

    #[test]
    fn bowtie_vertex_not_manifold() {
        // two triangles touching only at vertex 0
        let mesh = m(5, &[&[0, 1, 2], &[0, 3, 4]]);
        let t = Topology::new(&mesh);
        assert!(!t.vert_is_manifold(0));
        assert!(!t.vert_is_edge_pair(0)); // four edges
        assert!(!t.vert_is_manifold(9)); // loose vertex
    }

    #[test]
    fn find_double_same_and_reversed_winding() {
        let mesh = m(5, &[&[0, 1, 2], &[2, 1, 4], &[0, 2, 1], &[1, 2, 0]]);
        let t = Topology::new(&mesh);
        // face 0 -> first matching other face along its first edge (0,1): face 2 is its reverse, face 3 is a rotation
        assert_eq!(t.face_find_double(0), Some(2));
        assert_eq!(t.face_find_double(2), Some(3));
        assert_eq!(t.face_find_double(1), None); // shares edges but another vertex set
    }

    #[test]
    fn find_double_none_for_different_sets_and_same_edge_quads() {
        let mesh = m(6, &[&[0, 1, 2, 3], &[0, 1, 4, 5], &[0, 1, 2]]);
        let t = Topology::new(&mesh);
        assert_eq!(t.face_find_double(0), None);
        let cube = Mesh::cube(1.0);
        let tc = Topology::new(&cube);
        assert!((0..6).all(|f| tc.face_find_double(f).is_none()));
    }
}
