//! Face-region extrude of an edit-mesh selection (E on faces).
//!
//! Ported from `bmesh/operators/bmo_extrude.cc` `bmo_extrude_face_region_exec()` (318) with the duplicate step of
//! `bmo_dupe.cc` (`bmo_mesh_copy`, `bmo_edge_copy`):
//!  - every vertex of an input face is duplicated (in mesh vertex order) and every input face is copied onto the copies;
//!  - a region boundary edge (used by fewer than two input faces, `rlen < 2`) gets a side quad: with the input face running
//!    `a -> b` over it the quad is `(a, b, b', a')`, reversed when `use_normal_flip`;
//!  - when any input edge also borders a face that is not input (`delorig`), the original input faces and the vertices
//!    that only touch input faces through edges with two or more faces are deleted; otherwise the originals stay and are
//!    flipped (`!skip_input_flip`).
//! Not ported: `use_keep_orig`, edges excluded, ortho-edge dissolve, select history, loop attribute copy, wire edges
//! (`Mesh` has none). Side quads are ordered by input face, then edge, where Blender uses edge creation order.

use crate::mesh::Mesh;
pub use crate::mesh_extrude::Extruded;
use std::collections::HashMap;

/// `bmo_extrude_face_region_exec()` on the faces `faces` (indices, repeats and out of range ignored).
pub fn extrude_faces(mesh: &Mesh, faces: &[u32], use_normal_flip: bool) -> Extruded {
    let nv = mesh.verts.len();
    let mut input = vec![false; mesh.faces.len()];
    for &f in faces {
        if let Some(x) = input.get_mut(f as usize) {
            *x = true;
        }
    }
    let key = |a: u32, b: u32| (a.min(b), a.max(b));
    /* edge -> (faces using it, input faces using it) */
    let mut edge_use: HashMap<(u32, u32), (u32, u32)> = HashMap::new();
    for (i, f) in mesh.faces.iter().enumerate() {
        for k in 0..f.len() {
            let e = edge_use.entry(key(f[k], f[(k + 1) % f.len()])).or_default();
            e.0 += 1;
            e.1 += input[i] as u32;
        }
    }
    let delorig = edge_use.values().any(|&(all, inp)| inp > 0 && all > inp);

    /* Vertices to delete: touched only by input faces, and every edge there has two or more faces. */
    let mut used = vec![false; nv];
    let mut in_region = vec![false; nv];
    let mut outside = vec![false; nv];
    let mut shallow = vec![false; nv];
    for (i, f) in mesh.faces.iter().enumerate() {
        for k in 0..f.len() {
            let v = f[k] as usize;
            used[v] = true;
            if input[i] { in_region[v] = true } else { outside[v] = true }
            if edge_use[&key(f[k], f[(k + 1) % f.len()])].0 < 2 || edge_use[&key(f[(k + f.len() - 1) % f.len()], f[k])].0 < 2 {
                shallow[v] = true;
            }
        }
    }
    let delete: Vec<bool> = (0..nv).map(|v| delorig && used[v] && in_region[v] && !outside[v] && !shallow[v]).collect();
    let mut remap = vec![u32::MAX; nv];
    let mut out = Extruded::default();
    for v in 0..nv {
        if !delete[v] {
            remap[v] = out.mesh.verts.len() as u32;
            out.mesh.verts.push(mesh.verts[v]);
        }
    }
    /* Duplicates, in mesh vertex order. */
    let mut dup = vec![u32::MAX; nv];
    for v in 0..nv {
        if in_region[v] {
            out.mesh.verts.push(mesh.verts[v]);
            dup[v] = out.mesh.verts.len() as u32 - 1;
            out.new_verts.push(dup[v]);
        }
    }
    for (i, f) in mesh.faces.iter().enumerate() {
        if !input[i] {
            out.mesh.faces.push(f.iter().map(|&v| remap[v as usize]).collect());
        } else if !delorig {
            out.mesh.faces.push(f.iter().rev().map(|&v| remap[v as usize]).collect());
        }
    }
    let mut dup_edges: Vec<(u32, u32)> = Vec::new();
    for (i, f) in mesh.faces.iter().enumerate().filter(|(i, _)| input[*i]) {
        let _ = i;
        out.mesh.faces.push(f.iter().map(|&v| dup[v as usize]).collect());
        out.dup_faces.push(out.mesh.faces.len() as u32 - 1);
        for k in 0..f.len() {
            let (a, b) = (dup[f[k] as usize], dup[f[(k + 1) % f.len()] as usize]);
            if !dup_edges.iter().any(|&(x, y)| key(x, y) == key(a, b)) {
                dup_edges.push((a, b));
            }
        }
    }
    out.new_edges = dup_edges;
    for (i, f) in mesh.faces.iter().enumerate().filter(|(i, _)| input[*i]) {
        let _ = i;
        for k in 0..f.len() {
            let (a, b) = (f[k], f[(k + 1) % f.len()]);
            if edge_use[&key(a, b)].1 >= 2 {
                continue;
            }
            let (ra, rb) = (remap[a as usize], remap[b as usize]);
            let (da, db) = (dup[a as usize], dup[b as usize]);
            out.mesh.faces.push(if use_normal_flip { vec![rb, ra, da, db] } else { vec![ra, rb, db, da] });
            out.new_faces.push(out.mesh.faces.len() as u32 - 1);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top_face(m: &Mesh) -> u32 {
        (0..m.faces.len()).max_by(|&a, &b| m.face_normal(a)[2].partial_cmp(&m.face_normal(b)[2]).unwrap()).unwrap() as u32
    }

    #[test]
    fn extruding_one_cube_face_replaces_it_and_adds_four_sides() {
        let cube = Mesh::cube(2.0);
        let t = top_face(&cube);
        let r = extrude_faces(&cube, &[t], false);
        assert_eq!((r.mesh.verts.len(), r.mesh.faces.len()), (12, 10));
        assert_eq!((r.new_verts.len(), r.dup_faces.len(), r.new_faces.len(), r.new_edges.len()), (4, 1, 4, 4));
        assert!(r.mesh.edges().iter().all(|e| e.faces == 2), "closed manifold: {:?}", r.mesh.edges());
        let d = r.dup_faces[0] as usize;
        assert!(r.mesh.face_normal(d)[2] > 0.99, "the copy keeps the input normal");
    }

    #[test]
    fn use_normal_flip_reverses_the_sides() {
        let cube = Mesh::cube(2.0);
        let t = top_face(&cube);
        let a = extrude_faces(&cube, &[t], false);
        let b = extrude_faces(&cube, &[t], true);
        for (&x, &y) in a.new_faces.iter().zip(&b.new_faces) {
            let (mut p, q) = (a.mesh.faces[x as usize].clone(), &b.mesh.faces[y as usize]);
            p.reverse();
            let k = q.iter().position(|&v| v == p[0]).unwrap();
            assert!((0..4).all(|i| q[(k + i) % 4] == p[i]));
        }
    }

    #[test]
    fn a_lone_plane_keeps_and_flips_the_original() {
        let plane = Mesh::plane(2.0);
        let r = extrude_faces(&plane, &[0], false);
        assert_eq!((r.mesh.verts.len(), r.mesh.faces.len()), (8, 6));
        assert!(r.mesh.face_normal(0)[2] < -0.99, "original is flipped");
        assert!(r.mesh.face_normal(r.dup_faces[0] as usize)[2] > 0.99);
    }

    #[test]
    fn a_closed_region_without_outside_neighbours_keeps_originals_flipped() {
        let cube = Mesh::cube(2.0);
        let all: Vec<u32> = (0..cube.faces.len() as u32).collect();
        let r = extrude_faces(&cube, &all, false);
        assert_eq!((r.mesh.verts.len(), r.mesh.faces.len(), r.new_faces.len()), (16, 12, 0));
        assert!((0..6).all(|i| r.mesh.face_normal(i).iter().zip(cube.face_normal(i)).all(|(a, b)| (a + b).abs() < 1e-5)));
    }

    #[test]
    fn two_adjacent_faces_share_the_inner_edge_and_get_six_sides() {
        let cube = Mesh::cube(2.0);
        let t = top_face(&cube) as usize;
        let n = cube.faces[t].clone();
        let side = (0..cube.faces.len()).find(|&i| i != t && cube.faces[i].iter().filter(|v| n.contains(v)).count() == 2).unwrap();
        let r = extrude_faces(&cube, &[t as u32, side as u32], false);
        assert_eq!(r.new_faces.len(), 6);
        assert_eq!(r.new_verts.len(), 6);
        assert!(r.mesh.edges().iter().all(|e| e.faces == 2));
    }

    #[test]
    fn invalid_input_is_ignored() {
        let cube = Mesh::cube(2.0);
        let r = extrude_faces(&cube, &[99], false);
        assert_eq!(r.mesh, cube);
    }
}
