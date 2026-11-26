//! Edit-mesh undo step (`editmesh_undo.cc`: `UndoMesh`, `undomesh_from_editmesh`, `undomesh_to_editmesh`,
//! `MeshUndoStep`, `mesh_undosys_step_encode` / `_decode`).
//!
//! A step stores, for every object in edit-mode, a full copy of its mesh together with the select mode and shape key
//! number (`UndoMesh`), plus the scene tool-setting select mode (`MeshUndoStep_SceneData`). Encoding copies, so later
//! edits never alter a stored step; decoding hands back copies to restore, the first element being the active object.
//! Selection flags travel with the copy (Blender keeps them in `.select_*` mesh attributes; here as vertex indices).
//! Not ported: the array-store de-duplication (`USE_ARRAY_STORE`, only a memory optimisation), shape-key ID copies,
//! vertex group names and UV select settings (no such data in `Mesh`).

use crate::mesh::Mesh;

/// `UndoMesh`.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoMesh {
    pub mesh: Mesh,
    pub selected_verts: Vec<u32>,
    pub selectmode: u8,
    pub shapenr: i32,
    /// `undo_size`: bytes held by the copy (the uncompressed size; Blender's is the array-store size).
    pub undo_size: usize,
}

/// `undomesh_from_editmesh`: copies the edit mesh.
pub fn undomesh_from_editmesh(mesh: &Mesh, selected_verts: &[u32], selectmode: u8, shapenr: i32) -> UndoMesh {
    let undo_size = mesh.verts.len() * std::mem::size_of::<[f32; 3]>()
        + mesh.faces.iter().map(|f| (f.len() + 1) * std::mem::size_of::<u32>()).sum::<usize>()
        + selected_verts.len() * std::mem::size_of::<u32>();
    UndoMesh { mesh: mesh.clone(), selected_verts: selected_verts.to_vec(), selectmode, shapenr, undo_size }
}

/// `undomesh_to_editmesh`: the copy to restore into the edit mesh (stored data stays untouched).
pub fn undomesh_to_editmesh(um: &UndoMesh) -> UndoMesh {
    um.clone()
}

/// `MeshUndoStep_SceneData` (the part `Mesh` knows about).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneData {
    pub selectmode: u8,
}

/// `MeshUndoStep_Elem`: object identity (`obedit_ref`) plus its mesh copy.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshUndoElem {
    pub object: u64,
    pub data: UndoMesh,
}

/// `MeshUndoStep`.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshUndoStep {
    pub scene_data: SceneData,
    pub elems: Vec<MeshUndoElem>,
    /// `us->step.data_size`: sum of the element `undo_size`.
    pub data_size: usize,
}

/// `mesh_undosys_step_encode`: one element per edit-mode object `(object, mesh, selected verts, shapenr)`; the first
/// is the active one. Refused (None) without objects, as Blender's poll requires an edit-mode object.
pub fn step_encode(objects: &[(u64, &Mesh, &[u32], i32)], selectmode: u8) -> Option<MeshUndoStep> {
    if objects.is_empty() {
        return None;
    }
    let elems: Vec<MeshUndoElem> = objects
        .iter()
        .map(|&(object, mesh, sel, shapenr)| MeshUndoElem { object, data: undomesh_from_editmesh(mesh, sel, selectmode, shapenr) })
        .collect();
    let data_size = elems.iter().map(|e| e.data.undo_size).sum();
    Some(MeshUndoStep { scene_data: SceneData { selectmode }, elems, data_size })
}

/// `mesh_undosys_step_decode`: the element copies to restore (first is the active object) and the select mode to
/// put back into the tool settings.
pub fn step_decode(us: &MeshUndoStep) -> (Vec<(u64, UndoMesh)>, u8) {
    (us.elems.iter().map(|e| (e.object, undomesh_to_editmesh(&e.data))).collect(), us.scene_data.selectmode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip_equal() {
        let cube = Mesh::cube(2.0);
        let us = step_encode(&[(7, &cube, &[0, 3], 1)], 2).unwrap();
        let (elems, selectmode) = step_decode(&us);
        assert_eq!(selectmode, 2);
        assert_eq!(elems.len(), 1);
        assert_eq!(elems[0].0, 7);
        assert_eq!(elems[0].1.mesh, cube);
        assert_eq!((elems[0].1.selected_verts.clone(), elems[0].1.selectmode, elems[0].1.shapenr), (vec![0, 3], 2, 1));
    }

    #[test]
    fn later_edits_do_not_alter_the_step() {
        let mut mesh = Mesh::cube(1.0);
        let before = mesh.clone();
        let mut sel = vec![1u32];
        let us = step_encode(&[(1, &mesh, &sel, 0)], 1).unwrap();
        mesh.verts[0] = [9.0, 9.0, 9.0];
        mesh.faces.pop();
        sel.push(5);
        assert_eq!(us.elems[0].data.mesh, before);
        assert_eq!(us.elems[0].data.selected_verts, vec![1]);
        // decoded copies are independent too
        let (mut elems, _) = step_decode(&us);
        elems[0].1.mesh.verts.clear();
        assert_eq!(us.elems[0].data.mesh, before);
    }

    #[test]
    fn multi_object_size_and_active_first() {
        let (a, b) = (Mesh::plane(1.0), Mesh::cube(1.0));
        let us = step_encode(&[(2, &a, &[], 0), (3, &b, &[], 0)], 4).unwrap();
        assert_eq!(us.data_size, us.elems[0].data.undo_size + us.elems[1].data.undo_size);
        assert!(us.data_size > 0);
        assert_eq!(step_decode(&us).0[0].0, 2);
        assert!(step_encode(&[], 1).is_none());
    }
}
