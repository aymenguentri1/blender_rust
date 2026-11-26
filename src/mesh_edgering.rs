//! Edge ring of an edit-mesh edge, the selection `mesh.loopcut_slide` cuts along.
//!
//! Ported from `bmesh/intern/bmesh_walkers_impl.cc` `bmw_EdgeringWalker_begin()` (1437) / `_step()` (1509) as
//! `edgering_select()` in `editors/mesh/editmesh_loopcut.cc` (99) starts it: `BMW_DELIMIT_EDGE_RING_NGONS`, so only
//! quads are walked through. From a loop `l` (edge `e` in face `F`) a step crosses `e` to the radial face `G` and takes
//! the edge opposite `e` there (`l->radial_next->next->next`); when `G` is not a quad it keeps to `F` and takes the edge
//! opposite `e` in `F`. A step is refused onto a non-quad face, a non-manifold edge or a visited edge.
//! `begin` rewinds to one end of the ring (first loop of the edge, in face order), flips to the other face there when the
//! end face is no quad, and walks forward from it.
//! Not ported: seam / sharp / material delimiters and hidden-element masks (`Mesh` has neither), wire edges.

use crate::mesh::Mesh;
use std::collections::{HashMap, HashSet};

type Edge = (u32, u32);
/// A BMesh loop: face index and corner index (the edge runs from corner `k` to `k + 1`).
type Loop = (usize, usize);

fn key(a: u32, b: u32) -> Edge {
    (a.min(b), a.max(b))
}

struct Radial<'a> {
    mesh: &'a Mesh,
    loops: HashMap<Edge, Vec<Loop>>,
}

impl<'a> Radial<'a> {
    fn new(mesh: &'a Mesh) -> Self {
        let mut loops: HashMap<Edge, Vec<Loop>> = HashMap::new();
        for (f, face) in mesh.faces.iter().enumerate() {
            for k in 0..face.len() {
                let (a, b) = (face[k], face[(k + 1) % face.len()]);
                if a != b {
                    loops.entry(key(a, b)).or_default().push((f, k));
                }
            }
        }
        Radial { mesh, loops }
    }
    fn edge(&self, l: Loop) -> Edge {
        let f = &self.mesh.faces[l.0];
        key(f[l.1], f[(l.1 + 1) % f.len()])
    }
    fn len(&self, l: Loop) -> usize {
        self.mesh.faces[l.0].len()
    }
    /// `BM_edge_is_boundary() || BM_edge_is_manifold()`: one or two faces.
    fn edge_ok(&self, l: Loop) -> bool {
        matches!(self.loops[&self.edge(l)].len(), 1 | 2)
    }
    /// `l->radial_next`: the next loop around the edge (itself on a boundary edge).
    fn radial_next(&self, l: Loop) -> Loop {
        let ring = &self.loops[&self.edge(l)];
        let i = ring.iter().position(|&x| x == l).unwrap_or(0);
        ring[(i + 1) % ring.len()]
    }
    /// `l->next->next`: the opposite edge of a quad.
    fn opposite(&self, l: Loop) -> Loop {
        (l.0, (l.1 + 2) % self.len(l))
    }
    /// `bmw_EdgeringWalker_step()` with `delimit_ngon`: the loop the walk moves to, if any.
    fn step(&self, l: Loop, visited: &HashSet<Edge>) -> Option<Loop> {
        if !self.edge_ok(l) {
            return None;
        }
        let r = self.radial_next(l);
        let mut c = self.opposite(r);
        if self.len(c) != 4 || !self.edge_ok(c) {
            c = self.opposite(l);
        }
        let ok = self.len(c) == 4 && self.edge_ok(c) && !visited.contains(&self.edge(c));
        ok.then_some(c)
    }
    fn walk(&self, mut l: Loop, visited: &mut HashSet<Edge>, out: &mut Vec<Edge>) -> Loop {
        out.push(self.edge(l));
        while let Some(n) = self.step(l, visited) {
            visited.insert(self.edge(n));
            out.push(self.edge(n));
            l = n;
        }
        l
    }
}

/// `BMW_EDGERING` from the edge `(a, b)`: every edge of its ring, each once, in walk order. Empty when the edge is not in
/// the mesh.
pub fn edge_ring(mesh: &Mesh, a: u32, b: u32) -> Vec<(u32, u32)> {
    let r = Radial::new(mesh);
    let e = key(a, b);
    let Some(&start) = r.loops.get(&e).and_then(|v| v.first()) else {
        return Vec::new();
    };
    /* Rewind to one end. */
    let mut visited: HashSet<Edge> = HashSet::from([e]);
    let end = r.walk(start, &mut visited, &mut Vec::new());
    let mut l = end;
    if r.len(l) != 4 {
        l = r.radial_next(l);
    }
    visited = HashSet::from([r.edge(l)]);
    let mut out = Vec::new();
    /* Both sides of an end between two non-quads, so both directions are walked. */
    if r.radial_next(l) != l && r.len(l) != 4 {
        r.walk(r.radial_next(l), &mut visited, &mut out);
    }
    r.walk(l, &mut visited, &mut out);
    let mut seen = HashSet::new();
    out.retain(|e| seen.insert(*e));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut v: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
        v.sort();
        v
    }

    #[test]
    fn cube_edge_ring_is_the_four_parallel_edges() {
        let m = Mesh::cube(2.0);
        let e = m.edges()[0];
        let ring = edge_ring(&m, e.a, e.b);
        assert_eq!(ring.len(), 4);
        /* The four edges are parallel: same direction vector. */
        let d = |&(a, b): &(u32, u32)| {
            let (p, q) = (m.verts[a as usize], m.verts[b as usize]);
            [(q[0] - p[0]).abs() > 1e-3, (q[1] - p[1]).abs() > 1e-3, (q[2] - p[2]).abs() > 1e-3]
        };
        assert!(ring.iter().all(|x| d(x) == d(&ring[0])));
        assert!(ring.contains(&key(e.a, e.b)));
    }

    #[test]
    fn ring_is_the_same_from_any_of_its_edges() {
        let m = Mesh::cube(2.0);
        let e = m.edges()[3];
        let ring = sorted(edge_ring(&m, e.a, e.b));
        for &(a, b) in &ring {
            assert_eq!(sorted(edge_ring(&m, a, b)), ring);
        }
    }

    #[test]
    fn grid_ring_runs_across_to_the_boundary() {
        /* 4x4 faces: an edge across a row of four faces has a ring of the five parallel edges of that row. */
        let m = Mesh::grid(4, 4, 2.0);
        let e = m
            .edges()
            .into_iter()
            .find(|e| {
                let (p, q) = (m.verts[e.a as usize], m.verts[e.b as usize]);
                (p[0] - q[0]).abs() < 1e-4 && e.faces == 2
            })
            .unwrap();
        let ring = edge_ring(&m, e.a, e.b);
        assert_eq!(ring.len(), 5, "{ring:?}");
        assert!(ring.iter().all(|&(a, b)| (m.verts[a as usize][0] - m.verts[b as usize][0]).abs() < 1e-4));
    }

    #[test]
    fn closed_ring_visits_each_edge_once() {
        let m = Mesh::torus(8, 6, 1.0, 0.3);
        let e = m.edges()[0];
        let ring = edge_ring(&m, e.a, e.b);
        let mut s = ring.clone();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), ring.len());
        assert!(ring.len() == 8 || ring.len() == 6, "{}", ring.len());
    }

    #[test]
    fn triangle_neighbour_stops_the_ring_and_unknown_edge_is_empty() {
        let m = Mesh { verts: vec![[0.0; 3]; 5], faces: vec![vec![0, 1, 2, 3], vec![1, 0, 4]] };
        assert_eq!(sorted(edge_ring(&m, 0, 1)), vec![key(0, 1), key(2, 3)]);
        assert!(edge_ring(&m, 7, 9).is_empty());
    }
}
