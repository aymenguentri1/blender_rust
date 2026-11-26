//! `SCULPT_OT_face_sets_create` (sculpt_face_set.cc:356-575, `create_op_exec`): new face set from masked / visible /
//! all / selected faces. Mesh PBVH only; the multires (Grids) mask path and BMesh (dyntopo) are not ported.

use crate::face_set::{ensure, find_next_available_id};

/// `CreateMode` (sculpt_face_set.cc:356).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateMode {
    Masked,
    Visible,
    All,
    Selection,
}

/// Mesh attributes the operator reads. `None` is an absent attribute (`.hide_poly`, `.select_poly`, `.sculpt_mask`).
pub struct CreateInput<'a> {
    pub faces: &'a [Vec<u32>],
    pub hide_poly: Option<&'a [bool]>,
    pub select_poly: Option<&'a [bool]>,
    pub mask: Option<&'a [f32]>,
}

fn hidden(hide_poly: Option<&[bool]>, face: usize) -> bool {
    hide_poly.is_some_and(|h| h[face])
}

/// `create_op_exec()`: writes the new face set into `layer` and returns its id (`find_next_available_id`, taken
/// before any change). The layer is created lazily as in `ensure_face_sets_mesh()` whenever faces are updated.
pub fn create(
    mode: CreateMode,
    input: &CreateInput,
    layer: &mut Option<Vec<i32>>,
    color_default: &mut i32,
) -> i32 {
    let next = find_next_available_id(layer.as_deref());
    let n = input.faces.len();
    let hide = input.hide_poly;
    match mode {
        CreateMode::Masked => {
            /* No `.sculpt_mask` layer: nothing is updated, not even the face set layer. */
            let Some(mask) = input.mask.filter(|m| !m.is_empty()) else { return next };
            ensure(layer, n, color_default);
            for (f, verts) in input.faces.iter().enumerate() {
                if !hidden(hide, f) && verts.iter().any(|&v| mask[v as usize] > 0.5) {
                    layer.as_mut().unwrap()[f] = next;
                }
            }
        }
        CreateMode::Visible => {
            /* `booleans_mix_calc`: only a mix of hidden and visible faces creates a set; otherwise the layer is
             * removed (`clear_face_sets`, `color_default` is left alone). */
            let count = hide.map_or(0, |h| h.iter().filter(|&&b| b).count());
            if count == 0 || count == n {
                *layer = None;
            } else {
                ensure(layer, n, color_default);
                for f in (0..n).filter(|&f| !hidden(hide, f)) {
                    layer.as_mut().unwrap()[f] = next;
                }
            }
        }
        CreateMode::All => {
            ensure(layer, n, color_default);
            layer.as_mut().unwrap().fill(next);
        }
        CreateMode::Selection => {
            ensure(layer, n, color_default);
            for f in 0..n {
                if input.select_poly.is_some_and(|s| s[f]) && !hidden(hide, f) {
                    layer.as_mut().unwrap()[f] = next;
                }
            }
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quads() -> Vec<Vec<u32>> {
        vec![vec![0, 1, 2], vec![2, 3, 4], vec![4, 5, 6], vec![6, 7, 0]]
    }

    fn run(mode: CreateMode, hide: Option<&[bool]>, sel: Option<&[bool]>, mask: Option<&[f32]>, layer: Option<Vec<i32>>) -> (i32, Option<Vec<i32>>) {
        let faces = quads();
        let input = CreateInput { faces: &faces, hide_poly: hide, select_poly: sel, mask };
        let (mut l, mut d) = (layer, 9);
        let id = create(mode, &input, &mut l, &mut d);
        (id, l)
    }

    #[test]
    fn masked_takes_faces_with_a_vertex_above_half() {
        let mut mask = [0.0; 8];
        mask[3] = 0.6;
        mask[5] = 0.5; /* not > 0.5 */
        let (id, l) = run(CreateMode::Masked, Some(&[false, false, true, false]), None, Some(&mask), None);
        assert_eq!((id, l.unwrap()), (2, vec![1, 2, 1, 1]));
        let (id, l) = run(CreateMode::Masked, None, None, None, None);
        assert_eq!((id, l), (2, None), "no mask layer: nothing happens");
    }

    #[test]
    fn masked_skips_hidden_faces() {
        let mask = [1.0; 8];
        let (_, l) = run(CreateMode::Masked, Some(&[true, false, false, false]), None, Some(&mask), None);
        assert_eq!(l.unwrap(), [1, 2, 2, 2]);
    }

    #[test]
    fn visible_mixed_sets_visible_faces() {
        let (id, l) = run(CreateMode::Visible, Some(&[true, false, true, false]), None, None, Some(vec![1, 3, 1, 1]));
        assert_eq!((id, l.unwrap()), (4, vec![1, 4, 1, 4]));
    }

    #[test]
    fn visible_all_or_none_hidden_clears_the_layer() {
        for hide in [None, Some(&[false; 4][..]), Some(&[true; 4][..])] {
            let (_, l) = run(CreateMode::Visible, hide, None, None, Some(vec![1, 2, 3, 1]));
            assert_eq!(l, None);
        }
    }

    #[test]
    fn all_fills_and_creates_the_layer() {
        let (id, l) = run(CreateMode::All, None, None, None, None);
        assert_eq!((id, l.unwrap()), (2, vec![2; 4]));
        let (id, l) = run(CreateMode::All, None, None, None, Some(vec![1, 5, 1, 1]));
        assert_eq!((id, l.unwrap()), (6, vec![6; 4]));
    }

    #[test]
    fn selection_skips_hidden_and_creates_layer_even_without_selection() {
        let (id, l) = run(CreateMode::Selection, Some(&[false, true, false, false]), Some(&[true, true, false, true]), None, None);
        assert_eq!((id, l.unwrap()), (2, vec![2, 1, 1, 2]));
        let (_, l) = run(CreateMode::Selection, None, None, None, None);
        assert_eq!(l.unwrap(), [1; 4]);
    }
}
