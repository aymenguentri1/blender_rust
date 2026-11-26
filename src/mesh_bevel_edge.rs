//! Edit-mode bevel (Ctrl+B) of isolated manifold edges, one segment, Offset width.
//!
//! Ported from `bmesh/tools/bmesh_bevel.cc`: `build_boundary()` (3521) places, for a bevel vertex with a single
//! beveled edge, one boundary vertex on each neighbouring unbeveled edge using `offset_meet()` (see
//! `mesh_bevel_meet.rs`); the edge's two faces lose the edge to the offset edge and a quad fills the gap.
//! Supported: manifold edges (two faces) whose endpoints have valence 3 and touch no other beveled edge. Others are
//! returned in `skipped`. Not ported: segments > 1 (profile), vertex meshes for 2+ beveled edges, clamp overlap.

use crate::mesh::{Mesh, V3};
use crate::mesh_bevel_meet::{offset_meet, MeetEdge};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Beveled {
    pub mesh: Mesh,
    /// Beveled edges the builder cannot handle yet (vertex index pairs as given).
    pub skipped: Vec<(u32, u32)>,
    /// The new quad per beveled edge.
    pub new_faces: Vec<u32>,
}

fn run_dir(face: &[u32], a: u32, b: u32) -> bool {
    let n = face.len();
    (0..n).any(|i| face[i] == a && face[(i + 1) % n] == b)
}

/// Bevels `edges` by `width` (offset from the edge, measured along each adjacent face).
pub fn bevel_edges(mesh: &Mesh, edges: &[(u32, u32)], width: f32) -> Beveled {
    let nv = mesh.verts.len() as u32;
    let mut out = Beveled { mesh: mesh.clone(), ..Default::default() };
    let mut nbrs: HashMap<u32, HashSet<u32>> = HashMap::new();
    for f in &mesh.faces {
        for i in 0..f.len() {
            let (a, b) = (f[i], f[(i + 1) % f.len()]);
            nbrs.entry(a).or_default().insert(b);
            nbrs.entry(b).or_default().insert(a);
        }
    }
    let mut todo: Vec<(u32, u32)> = Vec::new();
    for &(a, b) in edges {
        let key = (a.min(b), a.max(b));
        if a != b && a < nv && b < nv && !todo.contains(&key) {
            todo.push(key);
        }
    }
    let ends = |e: &(u32, u32), v: u32| e.0 == v || e.1 == v;
    let mut ok: Vec<(u32, u32)> = Vec::new();
    for e in &todo {
        let faces: Vec<usize> =
            (0..mesh.faces.len()).filter(|&f| run_dir(&mesh.faces[f], e.0, e.1) || run_dir(&mesh.faces[f], e.1, e.0)).collect();
        let shared = todo.iter().any(|o| o != e && (ends(o, e.0) || ends(o, e.1)));
        let valence3 = [e.0, e.1].iter().all(|v| nbrs.get(v).map_or(0, |s| s.len()) == 3);
        if faces.len() == 2 && valence3 && !shared {
            ok.push(*e);
        } else {
            out.skipped.push(*e);
        }
    }
    // Boundary vertex per (bevel vertex, neighbour): `offset_meet` of the beveled edge and the neighbour edge in the
    // face that holds both.
    let mut boundary: HashMap<(u32, u32), u32> = HashMap::new();
    for e in &ok {
        for (v, u) in [(e.0, e.1), (e.1, e.0)] {
            for (fi, face) in mesh.faces.iter().enumerate() {
                let n = face.len();
                let Some(i) = face.iter().position(|&x| x == v) else { continue };
                let (prev, next) = (face[(i + n - 1) % n], face[(i + 1) % n]);
                if prev != u && next != u {
                    continue;
                }
                let w = if next == u { prev } else { next };
                let p = |i: u32| mesh.verts[i as usize];
                let (beveled, other) = (MeetEdge { other: p(u), offset: width }, MeetEdge { other: p(w), offset: 0.0 });
                // e1 is the edge on the `next` side, e2 on the `prev` side.
                let (e1, e2) = if next == u { (beveled, other) } else { (other, beveled) };
                let fno = mesh.face_normal(fi);
                let co: V3 = offset_meet(e1, e2, p(v), fno, Some(fno));
                out.mesh.verts.push(co);
                boundary.insert((v, w), out.mesh.verts.len() as u32 - 1);
            }
        }
    }
    let bev_vert = |v: u32| ok.iter().any(|e| ends(e, v));
    let mut faces: Vec<Vec<u32>> = Vec::with_capacity(mesh.faces.len() + ok.len());
    for face in &mesh.faces {
        let n = face.len();
        let mut f2 = Vec::with_capacity(n + 2);
        for i in 0..n {
            let v = face[i];
            if !bev_vert(v) {
                f2.push(v);
                continue;
            }
            let (prev, next) = (face[(i + n - 1) % n], face[(i + 1) % n]);
            match (boundary.get(&(v, prev)), boundary.get(&(v, next))) {
                (Some(&p), Some(&q)) => f2.extend([p, q]),
                (Some(&p), None) | (None, Some(&p)) => f2.push(p),
                (None, None) => f2.push(v),
            }
        }
        faces.push(f2);
    }
    for e in &ok {
        let f1 = mesh.faces.iter().find(|f| run_dir(f, e.0, e.1) || run_dir(f, e.1, e.0)).unwrap();
        // Name the edge so that the first face runs a -> b.
        let (a, b) = if run_dir(f1, e.0, e.1) { (e.0, e.1) } else { (e.1, e.0) };
        let side = |v: u32, forward: bool| -> u32 {
            let face = mesh.faces.iter().find(|f| run_dir(f, if forward { a } else { b }, if forward { b } else { a })).unwrap();
            let n = face.len();
            let i = face.iter().position(|&x| x == v).unwrap();
            let other = if v == a { b } else { a };
            let w = if face[(i + 1) % n] == other { face[(i + n - 1) % n] } else { face[(i + 1) % n] };
            boundary[&(v, w)]
        };
        let quad = vec![side(b, true), side(a, true), side(a, false), side(b, false)];
        faces.push(quad);
        out.new_faces.push(faces.len() as u32 - 1);
    }
    // The bevel vertices lost every face corner: drop them and renumber (`BM_vert_kill`).
    let used: HashSet<u32> = faces.iter().flatten().copied().collect();
    let mut remap: HashMap<u32, u32> = HashMap::new();
    let mut verts = Vec::new();
    for (i, &co) in out.mesh.verts.iter().enumerate() {
        if used.contains(&(i as u32)) {
            remap.insert(i as u32, verts.len() as u32);
            verts.push(co);
        }
    }
    out.mesh.verts = verts;
    out.mesh.faces = faces.iter().map(|f| f.iter().map(|v| remap[v]).collect()).collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{cross, dot, sub};

    fn volume(m: &Mesh) -> f32 {
        let mut v = 0.0;
        for f in &m.faces {
            for i in 1..f.len() - 1 {
                let (a, b, c) = (m.verts[f[0] as usize], m.verts[f[i] as usize], m.verts[f[i + 1] as usize]);
                v += dot(a, cross(b, c)) / 6.0;
            }
        }
        v
    }

    fn assert_closed(m: &Mesh) {
        let mut dir: HashMap<(u32, u32), u32> = HashMap::new();
        for f in &m.faces {
            for i in 0..f.len() {
                *dir.entry((f[i], f[(i + 1) % f.len()])).or_default() += 1;
            }
        }
        for (&(a, b), &c) in &dir {
            assert_eq!(c, 1, "directed edge {a}->{b} used {c}x");
            assert_eq!(dir.get(&(b, a)), Some(&1), "edge {a}-{b} has no opposite");
        }
    }

    fn cube_edge() -> (Mesh, (u32, u32)) {
        let m = Mesh::cube(2.0);
        let (a, b) = (m.verts.iter().position(|&v| v == [1.0, 1.0, -1.0]).unwrap(), m.verts.iter().position(|&v| v == [1.0, 1.0, 1.0]).unwrap());
        (m, (a as u32, b as u32))
    }

    #[test]
    fn one_cube_edge_gives_ten_verts_seven_faces_and_stays_closed() {
        let (m, e) = cube_edge();
        let r = bevel_edges(&m, &[e], 0.25);
        assert!(r.skipped.is_empty());
        assert_eq!((r.mesh.verts.len(), r.mesh.faces.len()), (10, 7));
        assert_closed(&r.mesh);
        assert_eq!(r.new_faces, vec![6]);
    }

    #[test]
    fn the_bevel_cuts_a_prism_of_the_expected_volume() {
        let (m, e) = cube_edge();
        let r = bevel_edges(&m, &[e], 0.25);
        // Removed prism: right triangle with legs = width over the edge length 2.
        let expect = 8.0 - 0.5 * 0.25 * 0.25 * 2.0;
        assert!((volume(&r.mesh) - expect).abs() < 1e-4, "{}", volume(&r.mesh));
    }

    #[test]
    fn boundary_vertices_slide_along_the_neighbour_edges() {
        let (m, e) = cube_edge();
        let r = bevel_edges(&m, &[e], 0.25);
        for v in &r.mesh.verts[6..] {
            let d = sub(*v, m.verts[e.0 as usize]);
            let d2 = sub(*v, m.verts[e.1 as usize]);
            assert!((dot(d, d).sqrt() - 0.25).abs() < 1e-5 || (dot(d2, d2).sqrt() - 0.25).abs() < 1e-5);
        }
    }

    #[test]
    fn adjacent_beveled_edges_and_open_edges_are_skipped() {
        let (m, e) = cube_edge();
        let c = m.verts.iter().position(|&v| v == [1.0, -1.0, 1.0]).unwrap() as u32;
        let r = bevel_edges(&m, &[e, (e.1, c)], 0.1);
        assert_eq!(r.skipped.len(), 2);
        assert_eq!(r.mesh, m);
        let p = Mesh::plane(2.0);
        assert_eq!(bevel_edges(&p, &[(0, 1)], 0.1).skipped, vec![(0, 1)]);
    }
}
