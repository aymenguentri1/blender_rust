//! Thumb brush (`mesh/brushes/thumb.cc`: `do_thumb_brush`, `calc_faces`, `translations_from_offset_and_factors`).
//! Every vertex under the brush moves by `offset * factor`, with
//! `offset = cross(cross(normal, grab_delta), normal) * bstrength` (the grab delta projected onto the surface plane).
//! Deviation: the caller supplies `grab_delta` (the stroke does not track `grab_delta_symm` yet).

use crate::mesh::{add, scale, V3};

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `cross(cross(sculpt_normal_symm, grab_delta_symm), sculpt_normal_symm) * bstrength`.
pub fn offset(normal: V3, grab_delta: V3, bstrength: f32) -> V3 {
    scale(cross(cross(normal, grab_delta), normal), bstrength)
}

/// One dab over `hit`, the `(vertex, brush factor)` pairs (`translations_from_offset_and_factors`).
pub fn dab(verts: &mut [V3], hit: &[(usize, f32)], offset: V3) {
    for &(i, f) in hit {
        verts[i] = add(verts[i], scale(offset, f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_is_the_delta_projected_onto_the_surface() {
        assert_eq!(offset([0., 0., 1.], [1., 0., 1.], 1.0), [1., 0., 0.]);
        assert_eq!(offset([0., 0., 1.], [2., 0., 0.], 0.5), [1., 0., 0.]);
        assert_eq!(offset([0., 0., 1.], [0., 0., 3.], 1.0), [0., 0., 0.]);
    }

    #[test]
    fn dab_scales_by_factor_and_skips_the_rest() {
        let mut v = vec![[0.0; 3], [1., 0., 0.], [2., 0., 0.]];
        dab(&mut v, &[(0, 1.0), (1, 0.5)], [0., 2., 0.]);
        assert_eq!(v[0], [0., 2., 0.]);
        assert_eq!(v[1], [1., 1., 0.]);
        assert_eq!(v[2], [2., 0., 0.]);
    }

    #[test]
    fn masked_vertices_have_zero_factor_and_stay() {
        let mut v = vec![[0.0; 3]];
        dab(&mut v, &[(0, 0.0)], [1., 1., 1.]);
        assert_eq!(v[0], [0.0; 3]);
    }
}
