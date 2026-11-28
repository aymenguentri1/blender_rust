//! Sculpt visibility filter: grow / shrink the visible area by one face ring per iteration.
//!
//! Ported from `editors/sculpt_paint/mesh/paint_hide.cc` (Mesh pbvh path only):
//!  - `PAINT_OT_visibility_filter` (`visibility_filter_exec()`): `action` GROW = `VisAction::Show`, SHRINK =
//!    `VisAction::Hide`; `iterations` 1..=100 (default 1); `auto_iteration_count` (default true) replaces the
//!    count with `num_verts / VERTEX_ITERATION_THRESHOLD + 1`
//!  - `grow_shrink_visibility_mesh()`: without `.hide_vert` nothing happens (all visible)
//!  - `propagate_vertex_visibility()` + `affect_visibility_mesh()`: only hidden faces are visited; every corner
//!    whose point already has the target value copies it to the previous and next corner points; two buffers
//!    alternate (`DualBuffer`), and after each iteration the faces are re-flushed (`flush_face_changes()`)
//!
//! `.hide_edge` is derived and not stored; undo and PBVH tagging are not ported. When `.hide_poly` is absent
//! Blender reads an all-false copy, so no face is visited (nothing propagates) while the flush still creates it.

#![allow(dead_code)] // not wired into the app yet

use crate::mesh::Mesh;
use crate::sculpt_hide::{VisAction, Visibility};

/// `VERTEX_ITERATION_THRESHOLD`: points per automatic iteration step.
pub const VERTEX_ITERATION_THRESHOLD: f32 = 50000.0;

/// RNA defaults of `PAINT_OT_visibility_filter` (`iterations` soft/hard range 1..=100).
pub const ITERATIONS_DEFAULT: i32 = 1;
pub const ITERATIONS_RANGE: (i32, i32) = (1, 100);

/// `iterations = int(num_verts / VERTEX_ITERATION_THRESHOLD) + 1`.
pub fn auto_iteration_count(num_verts: usize) -> i32 {
    (num_verts as f32 / VERTEX_ITERATION_THRESHOLD) as i32 + 1
}

/// `affect_visibility_mesh<value>()`.
fn affect_visibility(face: &[u32], value: bool, read: &[bool], write: &mut [bool]) {
    let n = face.len();
    for c in 0..n {
        if read[face[c] as usize] != value {
            continue;
        }
        write[face[(c + n - 1) % n] as usize] = value;
        write[face[(c + 1) % n] as usize] = value;
    }
}

/// `grow_shrink_visibility_mesh()`; `iterations` is used as given (clamped to at least 1 like the loop bound).
pub fn grow_shrink_visibility(mesh: &Mesh, vis: &mut Visibility, action: VisAction, iterations: i32) {
    let Some(hide_vert) = vis.hide_vert.as_ref() else { return };
    let iterations = iterations.max(1) as usize;
    let value = action == VisAction::Hide;
    /* `hide_poly` is a live span when the layer exists, otherwise an all-false copy. */
    let stale = vec![false; mesh.faces.len()];
    let mut bufs = [hide_vert.clone(), hide_vert.clone()]; /* [front, back] */
    for i in 0..iterations {
        let (read_idx, write_idx) = if i % 2 == 0 { (0, 1) } else { (1, 0) };
        let read = bufs[read_idx].clone();
        let write = &mut bufs[write_idx];
        {
            let hide_poly: &[bool] = vis.hide_face.as_deref().unwrap_or(&stale);
            for (fi, face) in mesh.faces.iter().enumerate() {
                if hide_poly[fi] {
                    affect_visibility(face, value, &read, write);
                }
            }
        }
        /* `flush_face_changes()`: `mesh_face_hide_from_vert()`. */
        vis.hide_face = Some(mesh.faces.iter().map(|f| f.iter().any(|&v| write[v as usize])).collect());
    }
    let last = if (iterations - 1) % 2 == 0 { 1 } else { 0 };
    vis.hide_vert = Some(bufs[last].clone());
}

/// `visibility_filter_exec()`: resolves `auto_iteration_count` and runs the filter.
pub fn visibility_filter(mesh: &Mesh, vis: &mut Visibility, action: VisAction, iterations: i32, auto: bool) {
    let n = if auto { auto_iteration_count(mesh.verts.len()) } else { iterations.clamp(ITERATIONS_RANGE.0, ITERATIONS_RANGE.1) };
    grow_shrink_visibility(mesh, vis, action, n);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt_hide::{hide_show_masked, VisAction};

    /// 1x4 strip of quads: points 0..=9 (bottom 0..=4, top 5..=9), faces i = [i, i+1, 6+i, 5+i].
    fn strip() -> Mesh {
        Mesh {
            verts: vec![[0.0; 3]; 10],
            faces: (0..4).map(|i| vec![i, i + 1, 6 + i, 5 + i]).collect(),
        }
    }

    /// Hide the points of columns `cols` (a point column c holds points c and c + 5).
    fn hidden_cols(m: &Mesh, cols: &[usize]) -> Visibility {
        let mut mask = vec![0.0; 10];
        for &c in cols {
            mask[c] = 1.0;
            mask[c + 5] = 1.0;
        }
        let mut v = Visibility::default();
        hide_show_masked(m, &mut v, VisAction::Hide, Some(&mask));
        v
    }

    #[test]
    fn auto_count_is_verts_over_threshold_plus_one() {
        assert_eq!(auto_iteration_count(10), 1);
        assert_eq!(auto_iteration_count(49_999), 1);
        assert_eq!(auto_iteration_count(50_000), 2);
        assert_eq!(auto_iteration_count(1_500_000), 31);
    }

    #[test]
    fn fully_visible_mesh_is_untouched() {
        let m = strip();
        let mut v = Visibility::default();
        grow_shrink_visibility(&m, &mut v, VisAction::Show, 3);
        grow_shrink_visibility(&m, &mut v, VisAction::Hide, 3);
        assert_eq!(v, Visibility::default());
    }

    #[test]
    fn grow_reveals_one_point_column_per_iteration() {
        let m = strip();
        let mut v = hidden_cols(&m, &[0, 1, 2]);
        assert_eq!(v.hide_face, Some(vec![true, true, true, false]));
        grow_shrink_visibility(&m, &mut v, VisAction::Show, 1);
        let hv = v.hide_vert.clone().unwrap();
        assert_eq!((0..5).map(|c| hv[c]).collect::<Vec<_>>(), [true, true, false, false, false]);
        assert_eq!(v.hide_face, Some(vec![true, true, false, false]));
    }

    #[test]
    fn grow_two_iterations_reveals_two_columns() {
        let m = strip();
        let mut v = hidden_cols(&m, &[0, 1, 2]);
        grow_shrink_visibility(&m, &mut v, VisAction::Show, 2);
        let hv = v.hide_vert.clone().unwrap();
        assert_eq!((0..5).map(|c| hv[c]).collect::<Vec<_>>(), [true, false, false, false, false]);
        assert_eq!(v.hide_face, Some(vec![true, false, false, false]));
    }

    #[test]
    fn shrink_hides_the_boundary_ring() {
        let m = strip();
        let mut v = hidden_cols(&m, &[0]);
        assert_eq!(v.hide_face, Some(vec![true, false, false, false]));
        grow_shrink_visibility(&m, &mut v, VisAction::Hide, 1);
        let hv = v.hide_vert.clone().unwrap();
        assert_eq!((0..5).map(|c| hv[c]).collect::<Vec<_>>(), [true, true, false, false, false]);
        assert_eq!(v.hide_face, Some(vec![true, true, false, false]));
        grow_shrink_visibility(&m, &mut v, VisAction::Hide, 2);
        let hv = v.hide_vert.clone().unwrap();
        assert_eq!((0..5).map(|c| hv[c]).collect::<Vec<_>>(), [true, true, true, true, false]);
    }

    #[test]
    fn operator_clamps_iterations_and_honours_auto() {
        let m = strip();
        let mut a = hidden_cols(&m, &[0, 1, 2]);
        visibility_filter(&m, &mut a, VisAction::Show, 100, true); /* 10 points: auto = 1 */
        let mut b = hidden_cols(&m, &[0, 1, 2]);
        grow_shrink_visibility(&m, &mut b, VisAction::Show, 1);
        assert_eq!(a, b);
        let mut c = hidden_cols(&m, &[0, 1, 2]);
        visibility_filter(&m, &mut c, VisAction::Show, 501, false); /* clamps to 100: even, see the next test */
        let mut d = hidden_cols(&m, &[0, 1, 2]);
        grow_shrink_visibility(&m, &mut d, VisAction::Show, 100);
        assert_eq!(c, d);
    }

    #[test]
    fn result_is_the_last_written_dual_buffer_even_when_nothing_changes_any_more() {
        /* Blender copies `write_buffer(iterations - 1)`: once every face is visible the iterations stop writing, so an
         * even count returns the older buffer, which still has the last ring hidden. */
        let m = strip();
        let mut odd = hidden_cols(&m, &[0, 1, 2]);
        grow_shrink_visibility(&m, &mut odd, VisAction::Show, 5);
        assert_eq!(odd.hide_vert, Some(vec![false; 10]));
        let mut even = hidden_cols(&m, &[0, 1, 2]);
        grow_shrink_visibility(&m, &mut even, VisAction::Show, 6);
        assert_eq!(even.hide_vert.unwrap()[0], true);
    }

    #[test]
    fn absent_hide_poly_propagates_nothing_but_creates_the_layer() {
        let m = strip();
        let mut v = Visibility { hide_vert: Some(vec![true, true, false, false, false, true, true, false, false, false]), hide_face: None };
        grow_shrink_visibility(&m, &mut v, VisAction::Show, 1);
        assert_eq!(v.hide_vert.as_ref().unwrap()[1], true);
        assert_eq!(v.hide_face, Some(vec![true, true, false, false]));
    }
}
