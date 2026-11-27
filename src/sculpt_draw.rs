//! Draw brush (`mesh/brushes/draw.cc`: `do_draw_brush`, `calc_faces`, `translations_from_offset_and_factors`; `brush_strength()`
//! for `SCULPT_BRUSH_TYPE_DRAW` in sculpt.cc). Every vertex under the brush moves along the area normal by
//! `normal * radius * bstrength * factor`, `bstrength = alpha² * flip * pressure * overlap`.
//! Deviations: pressure is 1 and the stroke overlap (`paint_stroke_integrate_overlap`) is not integrated, the caller
//! passes it (see `DEFAULT_OVERLAP`); object scale is 1 and the brush tilt is not applied.

use crate::mesh::{add, scale, V3};

/// Stand-in for `paint_runtime->overlap_factor`, tuned to the previous by-eye Draw strength (`0.06 * 0.5 / 0.5²`).
pub const DEFAULT_OVERLAP: f32 = 0.12;

/// `brush_strength()` for Draw: `alpha * flip * pressure * overlap` with `alpha` the squared root alpha.
pub fn brush_strength(root_alpha: f32, invert: bool, overlap: f32) -> f32 {
    root_alpha * root_alpha * if invert { -1.0 } else { 1.0 } * overlap
}

/// The offset of a full-factor vertex: `effective_normal * radius * bstrength`.
pub fn offset(normal: V3, radius: f32, bstrength: f32) -> V3 {
    scale(normal, radius * bstrength)
}

/// One dab over `hit`, the `(vertex, brush factor)` pairs under the brush (`translations_from_offset_and_factors`).
pub fn dab(verts: &mut [V3], hit: &[(usize, f32)], normal: V3, radius: f32, bstrength: f32) {
    let off = offset(normal, radius, bstrength);
    for &(i, f) in hit {
        verts[i] = add(verts[i], scale(off, f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_squares_alpha_and_flips() {
        assert_eq!(brush_strength(0.5, false, 1.0), 0.25);
        assert_eq!(brush_strength(0.5, true, 1.0), -0.25);
        assert!((brush_strength(0.5, false, DEFAULT_OVERLAP) - 0.03).abs() < 1e-6);
    }

    #[test]
    fn offset_follows_the_normal_radius_and_strength() {
        assert_eq!(offset([0.0, 0.0, 1.0], 2.0, 0.25), [0.0, 0.0, 0.5]);
    }

    #[test]
    fn dab_scales_by_factor_and_skips_the_rest() {
        let mut v = vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        dab(&mut v, &[(0, 1.0), (1, 0.5)], [0.0, 0.0, 1.0], 2.0, 0.25);
        assert_eq!(v[0], [0.0, 0.0, 0.5]);
        assert_eq!(v[1], [1.0, 0.0, 0.25]);
        assert_eq!(v[2], [2.0, 0.0, 0.0]);
        dab(&mut v, &[(0, 1.0)], [0.0, 0.0, 1.0], 2.0, -0.25);
        assert_eq!(v[0], [0.0; 3], "inverted digs back");
    }
}
