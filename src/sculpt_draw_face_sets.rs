//! Draw Face Sets brush (`mesh/brushes/draw_face_sets.cc`: `calc_faces`, `apply_face_set`;
//! `face_set::fill_factor_from_hide_and_mask` and `calc_face_centers` of sculpt_face_set.cc; `brush_strength()` for
//! `SCULPT_BRUSH_TYPE_DRAW_FACE_SETS`). Each face gets the factor `1 - mean vertex mask` (0 when hidden), its centre must lie
//! inside the radius, the brush falloff and strength scale the factor, and the face takes `face_set_id` when the factor is
//! above `FACE_SET_MIN_FADE`. Mesh path only (no Grids / BMesh); stroke wiring needs a face set layer on the object and is
//! left to the caller (`face_set::ensure` creates the layer).

use crate::face_set::MIN_FADE;
use crate::mesh::{dot, sub, V3};

/// `brush_strength()` for Draw Face Sets: `alpha * pressure * overlap`, `alpha` the squared root alpha, pressure 1.
pub fn brush_strength(root_alpha: f32, overlap: f32) -> f32 {
    root_alpha * root_alpha * overlap
}

/// `face_center_calc()`: the mean of the face vertices.
pub fn face_center(verts: &[V3], face: &[u32]) -> V3 {
    let mut sum = [0.0f32; 3];
    for &v in face {
        let p = verts[v as usize];
        sum = [sum[0] + p[0], sum[1] + p[1], sum[2] + p[2]];
    }
    let w = 1.0 / face.len().max(1) as f32;
    [sum[0] * w, sum[1] * w, sum[2] * w]
}

/// `fill_factor_from_hide_and_mask()` for one face; `mask` is per vertex, `hidden` is `.hide_poly` of the face.
pub fn face_factor(face: &[u32], mask: Option<&[f32]>, hidden: bool) -> f32 {
    if hidden {
        return 0.0;
    }
    match mask {
        Some(m) => 1.0 - face.iter().map(|&v| m[v as usize]).sum::<f32>() / face.len().max(1) as f32,
        None => 1.0,
    }
}

/// One dab: `layer` is the face set id of every face. Returns whether any face changed.
#[allow(clippy::too_many_arguments)]
pub fn dab(verts: &[V3], faces: &[Vec<u32>], mask: Option<&[f32]>, hide_poly: Option<&[bool]>, layer: &mut [i32], center: V3, radius: f32, strength: f32, face_set_id: i32, falloff: impl Fn(f32) -> f32) -> bool {
    let mut changed = false;
    for (i, face) in faces.iter().enumerate() {
        let d = sub(face_center(verts, face), center);
        let dist = dot(d, d).sqrt();
        if dist >= radius {
            continue;
        }
        let factor = face_factor(face, mask, hide_poly.is_some_and(|h| h[i])) * falloff(dist / radius) * strength;
        if factor > MIN_FADE && layer[i] != face_set_id {
            layer[i] = face_set_id;
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /* Two triangles: a around x = 0.33, b around x = 3.33. */
    fn quad_pair() -> (Vec<V3>, Vec<Vec<u32>>) {
        (vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [3.0, 1.0, 0.0]], vec![vec![0, 1, 2], vec![3, 4, 5]])
    }

    #[test]
    fn strength_squares_alpha() {
        assert_eq!(brush_strength(0.5, 1.0), 0.25);
    }

    #[test]
    fn centre_and_factor() {
        let (v, f) = quad_pair();
        let c = face_center(&v, &f[0]);
        assert!((c[0] - 1.0 / 3.0).abs() < 1e-6 && (c[1] - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(face_factor(&f[0], None, false), 1.0);
        assert_eq!(face_factor(&f[0], None, true), 0.0);
        assert!((face_factor(&f[0], Some(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0]), false) - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn only_faces_inside_the_radius_take_the_id() {
        let (v, f) = quad_pair();
        let mut layer = vec![1, 1];
        assert!(dab(&v, &f, None, None, &mut layer, [0.0; 3], 1.0, 1.0, 5, |_| 1.0));
        assert_eq!(layer, vec![5, 1]);
        assert!(!dab(&v, &f, None, None, &mut layer, [0.0; 3], 1.0, 1.0, 5, |_| 1.0), "nothing new to change");
    }

    #[test]
    fn hidden_masked_and_faint_faces_are_skipped() {
        let (v, f) = quad_pair();
        let mut layer = vec![1, 1];
        dab(&v, &f, None, Some(&[true, false]), &mut layer, [0.0; 3], 1.0, 1.0, 5, |_| 1.0);
        assert_eq!(layer, vec![1, 1], "hidden");
        dab(&v, &f, Some(&[1.0; 6]), None, &mut layer, [0.0; 3], 1.0, 1.0, 5, |_| 1.0);
        assert_eq!(layer, vec![1, 1], "fully masked");
        /* Factor must be above FACE_SET_MIN_FADE (0.05): strength 0.05 is not enough, 0.06 is. */
        dab(&v, &f, None, None, &mut layer, [0.0; 3], 1.0, 0.05, 5, |_| 1.0);
        assert_eq!(layer, vec![1, 1]);
        dab(&v, &f, None, None, &mut layer, [0.0; 3], 1.0, 0.06, 5, |_| 1.0);
        assert_eq!(layer, vec![5, 1]);
    }
}
