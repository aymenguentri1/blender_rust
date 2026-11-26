//! Delete Loose of an edit-mesh selection (`MESH_OT_delete_loose`).
//!
//! Ported from `editmesh_tools.cc` `edbm_delete_loose_exec()` (587) and `bm_face_is_loose()` (572), in Blender's order:
//!  - a flag is only used when the selection holds an element of that kind (`use_x && totelem_old_sel[k]`);
//!  - faces: a selected face whose every edge is a boundary edge (used by one face) is deleted (`DEL_FACES` keeps its edges);
//!  - edges: a selected wire edge (no face left, including the edges the deleted faces left behind) is deleted;
//!  - verts: a selected vertex without any edge left is deleted.
//!    `Mesh` has no wire edges, so wire edges that survive are not representable and are dropped from the result
//!    (their vertices stay). Selection flags are cleared by Blender; this pure function has no selection to clear.

use crate::mesh::Mesh;
use std::collections::HashSet;

fn key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// Returns the mesh without the loose selected faces / edges / vertices (see the module docs). Out of range indices
/// are ignored; the remaining vertices keep their order and faces are re-indexed.
pub fn delete_loose(
    mesh: &Mesh,
    verts: &[u32],
    edges: &[(u32, u32)],
    faces: &[u32],
    use_verts: bool,
    use_edges: bool,
    use_faces: bool,
) -> Mesh {
    let nv = mesh.verts.len();
    let use_verts = use_verts && verts.iter().any(|&v| (v as usize) < nv);
    let use_edges = use_edges && !edges.is_empty();
    let use_faces = use_faces && faces.iter().any(|&f| (f as usize) < mesh.faces.len());

    let mut uses: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
    for f in &mesh.faces {
        for k in 0..f.len() {
            *uses.entry(key(f[k], f[(k + 1) % f.len()])).or_default() += 1;
        }
    }
    /* Faces. */
    let mut dead_face = vec![false; mesh.faces.len()];
    if use_faces {
        for &fi in faces {
            let Some(f) = mesh.faces.get(fi as usize) else { continue };
            dead_face[fi as usize] = (0..f.len()).all(|k| uses[&key(f[k], f[(k + 1) % f.len()])] == 1);
        }
    }
    let mut live: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
    for (_, f) in mesh.faces.iter().enumerate().filter(|(i, _)| !dead_face[*i]) {
        for k in 0..f.len() {
            *live.entry(key(f[k], f[(k + 1) % f.len()])).or_default() += 1;
        }
    }
    /* Edges: every edge that existed stays as a wire edge once its faces are gone, until deleted. */
    let mut wire: HashSet<(u32, u32)> = uses.keys().filter(|k| !live.contains_key(k)).copied().collect();
    if use_edges {
        for &(a, b) in edges {
            wire.remove(&key(a, b));
        }
    }
    /* Verts. */
    let mut has_edge = vec![false; nv];
    for &(a, b) in live.keys().chain(wire.iter()) {
        has_edge[a as usize] = true;
        has_edge[b as usize] = true;
    }
    let mut dead_vert = vec![false; nv];
    if use_verts {
        for &v in verts {
            if (v as usize) < nv && !has_edge[v as usize] {
                dead_vert[v as usize] = true;
            }
        }
    }
    let mut remap = vec![u32::MAX; nv];
    let mut out = Mesh { verts: Vec::new(), faces: Vec::new() };
    for (i, p) in mesh.verts.iter().enumerate().filter(|(i, _)| !dead_vert[*i]) {
        remap[i] = out.verts.len() as u32;
        out.verts.push(*p);
    }
    for (_, f) in mesh.faces.iter().enumerate().filter(|(i, _)| !dead_face[*i]) {
        out.faces.push(f.iter().map(|&v| remap[v as usize]).collect());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two triangles sharing the edge 1-2, a lone triangle 4 5 6, and a loose vertex 7.
    fn mesh() -> Mesh {
        let p = |x: f32| [x, 0.0, 0.0];
        Mesh {
            verts: (0..8).map(|i| p(i as f32)).collect(),
            faces: vec![vec![0, 1, 2], vec![1, 3, 2], vec![4, 5, 6]],
        }
    }

    #[test]
    fn loose_vertices_are_removed_and_indices_compacted() {
        let m = delete_loose(&mesh(), &[7], &[], &[], true, true, true);
        assert_eq!(m.verts.len(), 7);
        assert_eq!(m.faces, mesh().faces);
    }

    #[test]
    fn vertices_used_by_faces_are_kept() {
        let all: Vec<u32> = (0..8).collect();
        let m = delete_loose(&mesh(), &all, &[], &[], true, false, false);
        assert_eq!(m.verts.len(), 7, "only vertex 7 has no edge");
        assert!(delete_loose(&mesh(), &[0, 1], &[], &[], true, true, true).verts.len() == 8);
    }

    #[test]
    fn a_lone_selected_face_is_removed_but_a_face_with_a_shared_edge_is_not() {
        let m = delete_loose(&mesh(), &[], &[], &[0, 2], false, false, true);
        assert_eq!(m.faces.len(), 3 - 1, "face 2 is loose (all edges boundary); face 0 shares edge 1-2");
        assert_eq!(m.verts.len(), 8, "its vertices stay (the edges remain as wire)");
    }

    #[test]
    fn faces_then_edges_then_verts_cascade_in_one_run() {
        let m0 = mesh();
        let sel_v: Vec<u32> = (0..8).collect();
        let sel_e = [(4, 5), (5, 6), (4, 6)];
        let m = delete_loose(&m0, &sel_v, &sel_e, &[2], true, true, true);
        assert_eq!(m.faces.len(), 2);
        assert_eq!(m.verts.len(), 4, "vertices 4 5 6 and 7 are gone");
    }

    #[test]
    fn flags_are_respected_and_unselected_kinds_are_off() {
        let m0 = mesh();
        let sel_e = [(4, 5), (5, 6), (4, 6)];
        let m = delete_loose(&m0, &[4, 5, 6], &sel_e, &[2], false, true, true);
        assert_eq!((m.faces.len(), m.verts.len()), (2, 8), "use_verts off: vertices stay");
        let m = delete_loose(&m0, &[7], &[], &[2], true, true, false);
        assert_eq!((m.faces.len(), m.verts.len()), (3, 7), "use_faces off: face stays");
    }
}
