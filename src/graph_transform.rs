//! Moving keys and handles in the Graph editor (the translation of `TransConvertType_Graph`).
//!
//! Ported from `source/blender/editors/transform/transform_convert_graph.cc`:
//!  - `graph_bezt_get_transform_selection()` (182): with handles shown a key is moved when its centre is selected and
//!    then both its handles go with it; a selected handle alone moves only that handle; with handles hidden the key
//!    selection decides for all three points;
//!  - `bezt_to_transdata()` / `flushTransGraphData()`: every moved point is its original position plus the delta;
//!  - `recalcData_graphedit()` (946): when a key now sits after the next one (`test_time_fcurve()`) the curve is
//!    re-sorted by `sort_time_fcurve()` (blenkernel/intern/fcurve.cc:1341), which also swaps handles that crossed to
//!    the wrong side of their key and clamps the one that is out of place.
//! Not ported: snapping, proportional editing, NLA and unit mapping, `BKE_fcurve_handles_recalc` (Auto handles are not
//! recomputed), the active keyframe index, tweaking one handle while dragging (`is_launch_event_drag`).

use crate::keyframe_select::{BezTriple, BEZT_FLAG_SELECT};

/// `graph_bezt_get_transform_selection()` without the drag tweak: `(left handle, key, right handle)`.
pub fn transform_selection(bezt: &BezTriple, use_handle: bool) -> (bool, bool, bool) {
    let sel = |f: u8| f & BEZT_FLAG_SELECT != 0;
    let key = sel(bezt.f2);
    let left = if use_handle { sel(bezt.f1) } else { key };
    let right = if use_handle { sel(bezt.f3) } else { key };
    /* Whenever we move the key, we also move both handles. */
    if key {
        (true, true, true)
    } else {
        (left, key, right)
    }
}

/// Moves the selected points of `orig` by `delta` (frame, value) into `bezts` (`td->iloc` plus the transform).
/// `bezts` and `orig` must have the same length; the points that are not moved keep their position from `orig`.
pub fn translate(bezts: &mut [BezTriple], orig: &[BezTriple], delta: [f32; 2], use_handle: bool) {
    for (b, o) in bezts.iter_mut().zip(orig) {
        let (left, key, right) = transform_selection(o, use_handle);
        for (i, moved) in [(0, left), (1, key), (2, right)] {
            b.vec[i] = o.vec[i];
            if moved {
                b.vec[i][0] += delta[0];
                b.vec[i][1] += delta[1];
            }
        }
    }
}

/// `fcu_test_selected()`: whether any point of the curve is selected.
pub fn any_selected(bezts: &[BezTriple]) -> bool {
    bezts.iter().any(BezTriple::is_sel_any)
}

/// `test_time_fcurve()`: some key comes after the next one.
pub fn test_time(bezts: &[BezTriple]) -> bool {
    bezts.windows(2).any(|w| w[0].vec[1][0] > w[1].vec[1][0])
}

/// `sort_time_fcurve()`: bubble sort by key time, then swap the handles that are both on the wrong side of their key
/// and clamp the others to it.
pub fn sort_time(bezts: &mut [BezTriple]) {
    let mut ok = true;
    while ok {
        ok = false;
        for a in 0..bezts.len().saturating_sub(1) {
            if bezts[a].vec[1][0] > bezts[a + 1].vec[1][0] {
                bezts.swap(a, a + 1);
                ok = true;
            }
        }
    }
    for b in bezts.iter_mut() {
        if b.vec[0][0] > b.vec[1][0] && b.vec[2][0] < b.vec[1][0] {
            let (l, r) = (b.vec[0], b.vec[2]);
            b.vec[0][0] = r[0];
            b.vec[0][1] = r[1];
            b.vec[2][0] = l[0];
            b.vec[2][1] = l[1];
        } else {
            b.vec[0][0] = b.vec[0][0].min(b.vec[1][0]);
            b.vec[2][0] = b.vec[2][0].max(b.vec[1][0]);
        }
    }
}

/// The part of `recalcData_graphedit()` that is ported: moves the selection by `delta`, then re-sorts the curve when the
/// move put a key out of order. Returns whether it sorted.
pub fn move_selection(bezts: &mut [BezTriple], orig: &[BezTriple], delta: [f32; 2], use_handle: bool) -> bool {
    if !any_selected(orig) {
        bezts.copy_from_slice(orig);
        return false;
    }
    translate(bezts, orig, delta, use_handle);
    let unsorted = test_time(bezts);
    if unsorted {
        sort_time(bezts);
    }
    unsorted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fcurve_eval::{evaluate_keyframes, BezTriple as EvalBez, Extend, Ipo};

    const S: u8 = BEZT_FLAG_SELECT;

    fn key(frame: f32, value: f32, f: [u8; 3]) -> BezTriple {
        BezTriple { vec: [[frame - 1.0, value, 0.0], [frame, value, 0.0], [frame + 1.0, value, 0.0]], f1: f[0], f2: f[1], f3: f[2] }
    }

    #[test]
    fn a_selected_key_takes_both_handles_along() {
        let orig = [key(1.0, 0.0, [0, S, 0]), key(10.0, 5.0, [0, 0, 0])];
        let mut out = orig;
        translate(&mut out, &orig, [2.0, 3.0], true);
        assert_eq!(out[0].vec, [[2.0, 3.0, 0.0], [3.0, 3.0, 0.0], [4.0, 3.0, 0.0]]);
        assert_eq!(out[1], orig[1]);
    }

    #[test]
    fn a_selected_handle_moves_alone_when_handles_are_shown() {
        let orig = [key(5.0, 1.0, [0, 0, S])];
        let mut out = orig;
        translate(&mut out, &orig, [0.5, 2.0], true);
        assert_eq!(out[0].vec[0], orig[0].vec[0]);
        assert_eq!(out[0].vec[1], orig[0].vec[1]);
        assert_eq!(out[0].vec[2], [6.5, 3.0, 0.0]);
        /* With the handles hidden only the key selection counts. */
        let mut hidden = orig;
        translate(&mut hidden, &orig, [0.5, 2.0], false);
        assert_eq!(hidden[0], orig[0]);
    }

    #[test]
    fn moving_a_key_changes_the_evaluated_animation() {
        let orig = [key(1.0, 0.0, [0, 0, 0]), key(10.0, 9.0, [0, S, 0])];
        let eval = |k: &[BezTriple], t: f32| {
            let v: Vec<EvalBez> = k.iter().map(|b| EvalBez { vec: b.vec.map(|p| [p[0], p[1]]), ipo: Ipo::Lin }).collect();
            evaluate_keyframes(&v, Extend::Constant, t)
        };
        assert!((eval(&orig, 5.5) - 4.5).abs() < 1e-4);
        let mut moved = orig;
        assert!(!move_selection(&mut moved, &orig, [0.0, 9.0], true));
        assert!((eval(&moved, 5.5) - 9.0).abs() < 1e-4, "{}", eval(&moved, 5.5));
        assert!((eval(&moved, 10.0) - 18.0).abs() < 1e-4);
    }

    #[test]
    fn crossing_the_next_key_sorts_the_curve() {
        let orig = [key(1.0, 0.0, [0, S, 0]), key(4.0, 1.0, [0, 0, 0]), key(8.0, 2.0, [0, 0, 0])];
        let mut out = orig;
        assert!(move_selection(&mut out, &orig, [5.0, 0.0], true));
        let times: Vec<f32> = out.iter().map(|b| b.vec[1][0]).collect();
        assert_eq!(times, vec![4.0, 6.0, 8.0]);
        assert_eq!(out[1].vec[1][1], 0.0, "the moved key keeps its value");
        assert!(!test_time(&out));
    }

    #[test]
    fn handles_on_the_wrong_side_are_swapped_or_clamped() {
        let mut k = [key(5.0, 0.0, [0, 0, 0])];
        k[0].vec[0] = [7.0, 1.0, 0.0];
        k[0].vec[2] = [3.0, 2.0, 0.0];
        sort_time(&mut k);
        assert_eq!((k[0].vec[0][..2].to_vec(), k[0].vec[2][..2].to_vec()), (vec![3.0, 2.0], vec![7.0, 1.0]));
        /* Only one is out of place: it is clamped at the key. */
        let mut c = [key(5.0, 0.0, [0, 0, 0])];
        c[0].vec[0] = [8.0, 1.0, 0.0];
        sort_time(&mut c);
        assert_eq!((c[0].vec[0][0], c[0].vec[2][0]), (5.0, 6.0));
    }

    #[test]
    fn nothing_selected_changes_nothing() {
        let orig = [key(1.0, 0.0, [0, 0, 0]), key(2.0, 1.0, [0, 0, 0])];
        let mut out = orig;
        assert!(!move_selection(&mut out, &orig, [9.0, 9.0], true));
        assert_eq!(out, orig);
    }
}
