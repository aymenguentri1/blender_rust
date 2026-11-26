//! Merge of an edit-mesh vertex selection (`MESH_OT_merge`: At Center, At Cursor, At First, At Last).
//!
//! Ported from `editmesh_tools.cc` `merge_target()` (3407), `merge_firstlast()` (3351) and `bmo_pointmerge_exec()`
//! (`bmo_removedoubles.cc`, 465) followed by `weld_verts`:
//!  - Center: the mean of the selected vertices (summed in mesh order, `1/n`); At Cursor: a given point (the caller has
//!    already moved the cursor to object space, as `merge_target` does with `world_to_object`);
//!  - without an explicit target the first selected vertex (mesh order) takes the position and every other selected
//!    vertex is welded to it; with First/Last the given vertex keeps its position and the others weld to it
//!    (`merge_firstlast` returns false, so nothing happens, when that vertex is not selected);
//!  - welding removes the merged vertices, collapses consecutive duplicates in faces and drops faces left with fewer
//!    than three distinct vertices.
//!    Not ported: UV merge (`uvs`), select history (the caller picks the First/Last vertex), `weld_verts` face splitting
//!    for non-adjacent duplicates (such faces are dropped, as in `Mesh::merged_by_distance`).

use crate::mesh::Mesh;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MergeTarget {
    /// `AT_CENTER`.
    Center,
    /// `AT_CURSOR`, in object space.
    At([f32; 3]),
    /// `AT_FIRST` / `AT_LAST`: the select-history vertex.
    Vertex(u32),
}

/// Merges the vertices `selected` (indices; repeats and out of range ignored). Returns the new mesh and how many vertices
/// were removed, or `None` where Blender returns false (nothing selected, or the target vertex is not selected).
pub fn merge_selected(mesh: &Mesh, selected: &[u32], target: MergeTarget) -> Option<(Mesh, usize)> {
    let nv = mesh.verts.len();
    let mut sel = vec![false; nv];
    for &v in selected {
        if let Some(s) = sel.get_mut(v as usize) {
            *s = true;
        }
    }
    let order: Vec<usize> = (0..nv).filter(|&i| sel[i]).collect();
    if order.is_empty() {
        return None;
    }
    let mut verts = mesh.verts.clone();
    let (keep, co) = match target {
        MergeTarget::Vertex(t) => {
            if !sel.get(t as usize).copied().unwrap_or(false) {
                return None;
            }
            (t as usize, verts[t as usize])
        }
        MergeTarget::Center => {
            let mut c = [0.0f32; 3];
            for &i in &order {
                for k in 0..3 {
                    c[k] += verts[i][k];
                }
            }
            let fac = 1.0 / order.len() as f32;
            (order[0], [c[0] * fac, c[1] * fac, c[2] * fac])
        }
        MergeTarget::At(p) => (order[0], p),
    };
    verts[keep] = co;
    let mut to: Vec<usize> = (0..nv).collect();
    for &i in &order {
        to[i] = keep;
    }
    let mut remap = vec![u32::MAX; nv];
    let mut out = Mesh { verts: Vec::new(), faces: Vec::new() };
    for i in 0..nv {
        if to[i] == i {
            remap[i] = out.verts.len() as u32;
            out.verts.push(verts[i]);
        }
    }
    for f in &mesh.faces {
        let mut nf: Vec<u32> = Vec::with_capacity(f.len());
        for &i in f {
            let t = remap[to[i as usize]];
            if nf.last() != Some(&t) {
                nf.push(t);
            }
        }
        while nf.len() > 1 && nf.first() == nf.last() {
            nf.pop();
        }
        let mut uniq = nf.clone();
        uniq.sort_unstable();
        uniq.dedup();
        if nf.len() >= 3 && uniq.len() == nf.len() {
            out.faces.push(nf);
        }
    }
    let removed = nv - out.verts.len();
    Some((out, removed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad() -> Mesh {
        Mesh {
            verts: vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]],
            faces: vec![vec![0, 1, 2, 3]],
        }
    }

    #[test]
    fn at_center_collapses_to_the_mean_and_the_count_drops_by_n_minus_one() {
        let (m, removed) = merge_selected(&quad(), &[0, 1], MergeTarget::Center).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(m.verts.len(), 3);
        assert_eq!(m.verts[0], [1.0, 0.0, 0.0]);
        assert_eq!(m.faces, vec![vec![0, 1, 2]], "the quad becomes a triangle");
    }

    #[test]
    fn first_and_last_keep_the_chosen_vertex_position() {
        let (m, _) = merge_selected(&quad(), &[0, 1, 2], MergeTarget::Vertex(2)).unwrap();
        assert_eq!(m.verts.len(), 2);
        assert!(m.verts.contains(&[2.0, 2.0, 0.0]) && !m.verts.contains(&[0.0, 0.0, 0.0]));
        assert!(m.faces.is_empty(), "everything collapsed to an edge: the degenerate face is dropped");
        let (m, _) = merge_selected(&quad(), &[0, 1], MergeTarget::Vertex(1)).unwrap();
        assert_eq!(m.verts[0], [2.0, 0.0, 0.0]);
    }

    #[test]
    fn at_cursor_moves_the_merged_vertex_to_the_point() {
        let (m, removed) = merge_selected(&quad(), &[2, 3], MergeTarget::At([5.0, 5.0, 5.0])).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(m.verts[2], [5.0, 5.0, 5.0]);
    }

    #[test]
    fn nothing_selected_or_unselected_target_does_nothing() {
        assert!(merge_selected(&quad(), &[], MergeTarget::Center).is_none());
        assert!(merge_selected(&quad(), &[9], MergeTarget::Center).is_none());
        assert!(merge_selected(&quad(), &[0, 1], MergeTarget::Vertex(3)).is_none());
    }

    #[test]
    fn a_single_selected_vertex_only_moves_for_at_cursor() {
        let (m, removed) = merge_selected(&quad(), &[1], MergeTarget::Center).unwrap();
        assert_eq!((removed, m.verts[1]), (0, [2.0, 0.0, 0.0]));
        assert_eq!(m.faces, quad().faces);
    }
}
