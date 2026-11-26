//! Flip Normals of an edit-mesh face selection (`MESH_OT_flip_normals`, without `only_clnors`).
//!
//! Ported from `editmesh_tools.cc` `edbm_flip_normals_exec()` (2318) -> `edbm_flip_normals_face_winding()` (2281) ->
//! the `reverse_faces` bmesh operator (`bmo_utils.cc` `bmo_reverse_faces_exec()`, 156) -> `BM_face_normal_flip_ex()` ->
//! `bmesh_kernel_loop_reverse()` (`bmesh_core.cc`, 1094). The loop cycle is reversed with `f->l_first` kept, so a face
//! `v0 v1 .. vn-1` becomes `v0 vn-1 .. v1`: it starts at the same vertex.
//!    Not ported: custom loop normals (`only_clnors`, `flip_custom_normals`), multires displacement flip.

use crate::mesh::Mesh;

/// `reverse_faces` on the faces `faces` (indices; repeats and out of range are ignored). Returns the new mesh and how
/// many faces were flipped. Like the operator (`bm->totfacesel == 0` -> `continue`) an empty selection returns the mesh
/// unchanged.
pub fn flip_normals(mesh: &Mesh, faces: &[u32]) -> (Mesh, usize) {
    let mut out = mesh.clone();
    let mut done = vec![false; mesh.faces.len()];
    let mut n = 0;
    for &f in faces {
        let Some(flag) = done.get_mut(f as usize) else { continue };
        if std::mem::replace(flag, true) {
            continue;
        }
        let face = &mut out.faces[f as usize];
        if face.len() > 1 {
            face[1..].reverse();
        }
        n += 1;
    }
    (out, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> Mesh {
        Mesh::cube(1.0)
    }

    #[test]
    fn a_flipped_face_keeps_its_first_vertex_and_reverses_the_rest() {
        let mut m = cube();
        m.faces = vec![vec![0, 1, 2, 3], vec![4, 5, 6]];
        let (f, n) = flip_normals(&m, &[0]);
        assert_eq!(n, 1);
        assert_eq!(f.faces[0], vec![0, 3, 2, 1]);
        assert_eq!(f.faces[1], vec![4, 5, 6], "the unselected face is untouched");
    }

    #[test]
    fn positions_and_counts_are_unchanged_and_a_double_flip_is_the_identity() {
        let m = cube();
        let (f, n) = flip_normals(&m, &[0, 2, 4]);
        assert_eq!(n, 3);
        assert_eq!(f.verts, m.verts);
        assert_eq!(f.faces.len(), m.faces.len());
        assert_ne!(f.faces, m.faces);
        assert_eq!(flip_normals(&f, &[0, 2, 4]).0.faces, m.faces);
    }

    #[test]
    fn repeats_out_of_range_and_empty_selections_are_ignored() {
        let m = cube();
        let (f, n) = flip_normals(&m, &[1, 1, 999]);
        assert_eq!(n, 1, "face 1 flips once, 999 does not exist");
        assert_eq!(f.faces[1], {
            let mut r = m.faces[1].clone();
            r[1..].reverse();
            r
        });
        let (g, n) = flip_normals(&m, &[]);
        assert_eq!((n, g.faces), (0, m.faces));
    }
}
