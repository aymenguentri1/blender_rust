//! Sculpt mode hide / show: hide everything, show everything, hide / show by mask, invert.
//!
//! Ported from `editors/sculpt_paint/mesh/paint_hide.cc` (Mesh pbvh path only; grids and BMesh are not ported):
//!  - `partialvis_all_update_mesh()` (471): Hide sets `.hide_vert` everywhere; Show with no `.hide_vert` does nothing
//!  - `mesh_show_all()` (123): removes `.hide_vert`, then `mesh_hide_vert_flush()` drops `.hide_edge` / `.hide_poly`
//!  - `partialvis_masked_update_mesh()` (567): points with `.sculpt_mask > 0.5` take the action; Show without a mask
//!    layer is show all; with no mask layer Hide does nothing
//!  - `vert_hide_update()` (288) + `flush_face_changes_node()` (222): when a point changed, a face is hidden when any
//!    of its points is (`calc_face_hide()`, 201)
//!  - `invert_visibility_mesh()` (722): toggles `.hide_poly` on every face, then `mesh_hide_face_flush()`
//!    (blenkernel/intern/mesh_evaluate.cc:553): no hidden face removes `.hide_vert`; else points of hidden faces are
//!    hidden and points of visible faces shown (points without a face keep their value, `false` when new)
//!
//! Edge visibility (`.hide_edge`) is derived from the points and is not stored. Undo and PBVH tagging are not ported.

#![allow(dead_code)] // not wired into the app yet

use crate::mesh::Mesh;

/// `VisAction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisAction {
    Hide,
    Show,
}

/// The `.hide_vert` and `.hide_poly` attributes; `None` is "attribute absent", meaning everything is visible.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Visibility {
    pub hide_vert: Option<Vec<bool>>,
    pub hide_face: Option<Vec<bool>>,
}

/// `mesh_face_hide_from_vert()` (mesh_evaluate.cc:512): a face is hidden when any of its points is.
fn face_hide_from_vert(mesh: &Mesh, hide_vert: &[bool]) -> Vec<bool> {
    mesh.faces.iter().map(|f| f.iter().any(|&v| hide_vert[v as usize])).collect()
}

/// `vert_hide_update()`: the attribute is created (all visible) even when nothing changes; faces are re-flushed only
/// when a point changed.
fn vert_hide_update(mesh: &Mesh, vis: &mut Visibility, calc: impl Fn(usize, &mut bool)) {
    let hv = vis.hide_vert.get_or_insert_with(|| vec![false; mesh.verts.len()]);
    let mut changed = false;
    for (i, h) in hv.iter_mut().enumerate() {
        let old = *h;
        calc(i, h);
        changed |= old != *h;
    }
    if changed {
        vis.hide_face = Some(face_hide_from_vert(mesh, hv));
    }
}

/// `mesh_show_all()`.
fn show_all_mesh(vis: &mut Visibility) {
    vis.hide_vert = None;
    vis.hide_face = None;
}

/// `partialvis_all_update_mesh()`: `PAINT_OT_hide_show_all`.
pub fn hide_show_all(mesh: &Mesh, vis: &mut Visibility, action: VisAction) {
    match action {
        VisAction::Hide => vert_hide_update(mesh, vis, |_, h| *h = true),
        VisAction::Show => {
            if vis.hide_vert.is_some() {
                show_all_mesh(vis);
            }
        }
    }
}

/// `partialvis_masked_update_mesh()`: `PAINT_OT_hide_show_masked`. `mask` is the `.sculpt_mask` layer, empty or `None`
/// when absent.
pub fn hide_show_masked(mesh: &Mesh, vis: &mut Visibility, action: VisAction, mask: Option<&[f32]>) {
    if action == VisAction::Show && vis.hide_vert.is_none() {
        return;
    }
    let value = action == VisAction::Hide;
    let mask = mask.filter(|m| !m.is_empty());
    match (action, mask) {
        (VisAction::Show, None) => show_all_mesh(vis),
        (_, Some(m)) => vert_hide_update(mesh, vis, |i, h| {
            if m.get(i).is_some_and(|&x| x > 0.5) {
                *h = value;
            }
        }),
        (VisAction::Hide, None) => {}
    }
}

/// `invert_visibility_mesh()`: `PAINT_OT_visibility_invert`.
pub fn invert_visibility(mesh: &Mesh, vis: &mut Visibility) {
    let hf = vis.hide_face.get_or_insert_with(|| vec![false; mesh.faces.len()]);
    hf.iter_mut().for_each(|h| *h = !*h);
    /* `mesh_hide_face_flush()` */
    if !hf.iter().any(|&h| h) {
        vis.hide_vert = None;
        return;
    }
    let hv = vis.hide_vert.get_or_insert_with(|| vec![false; mesh.verts.len()]);
    for (f, &hidden) in mesh.faces.iter().zip(hf.iter()) {
        if hidden {
            f.iter().for_each(|&v| hv[v as usize] = true);
        }
    }
    for (f, &hidden) in mesh.faces.iter().zip(hf.iter()) {
        if !hidden {
            f.iter().for_each(|&v| hv[v as usize] = false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two quads sharing the edge 1-2: points 0..=5.
    fn strip() -> Mesh {
        Mesh {
            verts: vec![[0.0; 3]; 6],
            faces: vec![vec![0, 1, 2, 3], vec![1, 4, 5, 2]],
        }
    }

    #[test]
    fn hide_all_hides_every_point_and_face_and_show_all_removes_the_layers() {
        let m = strip();
        let mut v = Visibility::default();
        hide_show_all(&m, &mut v, VisAction::Hide);
        assert_eq!(v.hide_vert, Some(vec![true; 6]));
        assert_eq!(v.hide_face, Some(vec![true; 2]));
        hide_show_all(&m, &mut v, VisAction::Show);
        assert_eq!(v, Visibility::default());
    }

    #[test]
    fn show_without_a_hide_layer_does_nothing() {
        let m = strip();
        let mut v = Visibility { hide_vert: None, hide_face: Some(vec![true, false]) };
        hide_show_all(&m, &mut v, VisAction::Show);
        assert_eq!(v.hide_face, Some(vec![true, false]));
        hide_show_masked(&m, &mut v, VisAction::Show, None);
        assert_eq!(v.hide_face, Some(vec![true, false]));
    }

    #[test]
    fn hide_masked_uses_the_half_threshold_and_flushes_faces_by_any_point() {
        let m = strip();
        let mut v = Visibility::default();
        hide_show_masked(&m, &mut v, VisAction::Hide, Some(&[0.0, 0.5, 0.0, 0.0, 0.51, 0.0]));
        assert_eq!(v.hide_vert, Some(vec![false, false, false, false, true, false]));
        assert_eq!(v.hide_face, Some(vec![false, true]));
        /* Show by mask only reveals the masked points again. */
        hide_show_masked(&m, &mut v, VisAction::Show, Some(&[0.0, 0.0, 0.0, 0.0, 1.0, 0.0]));
        assert_eq!(v.hide_vert, Some(vec![false; 6]));
        assert_eq!(v.hide_face, Some(vec![false; 2]));
    }

    #[test]
    fn masked_without_a_mask_layer() {
        let m = strip();
        let mut v = Visibility::default();
        hide_show_masked(&m, &mut v, VisAction::Hide, None);
        assert_eq!(v, Visibility::default(), "nothing to hide");
        hide_show_all(&m, &mut v, VisAction::Hide);
        hide_show_masked(&m, &mut v, VisAction::Show, Some(&[]));
        assert_eq!(v, Visibility::default(), "show without a mask is show all");
    }

    #[test]
    fn a_hide_that_changes_nothing_still_creates_the_point_layer() {
        let m = strip();
        let mut v = Visibility::default();
        hide_show_masked(&m, &mut v, VisAction::Hide, Some(&[0.0; 6]));
        assert_eq!(v.hide_vert, Some(vec![false; 6]));
        assert_eq!(v.hide_face, None);
    }

    #[test]
    fn invert_flips_faces_and_flushes_points_with_visible_faces_winning() {
        let m = strip();
        let mut v = Visibility { hide_vert: None, hide_face: Some(vec![true, false]) };
        invert_visibility(&m, &mut v);
        assert_eq!(v.hide_face, Some(vec![false, true]));
        /* Points 1 and 2 touch the visible face 0, so they stay visible. */
        assert_eq!(v.hide_vert, Some(vec![false, false, false, false, true, true]));
        invert_visibility(&m, &mut v);
        assert_eq!(v.hide_face, Some(vec![true, false]));
        assert_eq!(v.hide_vert, Some(vec![true, false, false, true, false, false]));
    }

    #[test]
    fn invert_of_everything_visible_hides_everything_and_back() {
        let m = strip();
        let mut v = Visibility::default();
        invert_visibility(&m, &mut v);
        assert_eq!(v.hide_face, Some(vec![true; 2]));
        assert_eq!(v.hide_vert, Some(vec![true; 6]));
        invert_visibility(&m, &mut v);
        assert_eq!(v.hide_vert, None);
        assert_eq!(v.hide_face, Some(vec![false; 2]));
    }
}
