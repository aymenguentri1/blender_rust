//! Crease brush (`mesh/brushes/crease.cc`: `do_crease_or_blob_brush`, `calc_faces`; strength from `brush_strength()` in
//! sculpt.cc, `project_translations()` in sculpt.cc). Vertices are pulled toward the brush centre scaled by
//! `strength`, the component along the area normal is removed (so they pinch to a line, not a point), then the
//! whole is offset along the normal by `radius * bstrength`. Deviations: object scale is 1, the Tube falloff and the
//! cube tip are not ported; overlap and pressure are 1.

use crate::mesh::{add, dot, scale, sub, V3};

/// `Brush::crease_pinch_factor` (DNA_brush_types.h).
pub const CREASE_PINCH_FACTOR: f32 = 0.5;

/// `brush_strength()` for `SCULPT_BRUSH_TYPE_CREASE`: `alpha² * flip`, `flip` is -1 when inverted.
pub fn brush_strength(alpha: f32, invert: bool) -> f32 {
    alpha * alpha * if invert { -1.0 } else { 1.0 }
}

/// The pinch `strength` of `do_crease_or_blob_brush()` for crease (`invert_strength` false): `|bstrength|` times
/// `pinch² / alpha²` (the squared alpha divided out when positive).
pub fn pinch_strength(alpha: f32, invert: bool, pinch_factor: f32) -> f32 {
    let mut correction = pinch_factor * pinch_factor;
    if alpha > 0.0 {
        correction /= alpha * alpha;
    }
    brush_strength(alpha, invert).abs() * correction
}

/// `project_translations()`: removes the component of `t` along `plane` (`project_plane_v3_v3v3`).
pub fn project_translation(t: V3, plane: V3) -> V3 {
    let len_sq = dot(plane, plane);
    if len_sq < f32::EPSILON {
        return t;
    }
    add(t, scale(plane, dot(t, plane) * -(1.0 / len_sq)))
}

/// One dab; `falloff` maps the distance in brush radii (0..1) to the factor.
#[allow(clippy::too_many_arguments)]
pub fn dab(verts: &mut [V3], mask: &[f32], area_normal: V3, center: V3, radius: f32, alpha: f32, invert: bool, pinch_factor: f32, falloff: impl Fn(f32) -> f32) {
    let offset = scale(area_normal, radius * brush_strength(alpha, invert));
    let strength = pinch_strength(alpha, invert, pinch_factor);
    for (i, v) in verts.iter_mut().enumerate() {
        let d = sub(*v, center);
        let dist = dot(d, d).sqrt();
        if dist >= radius {
            continue;
        }
        let factor = falloff(dist / radius) * (1.0 - mask.get(i).copied().unwrap_or(0.0)).clamp(0.0, 1.0);
        if factor == 0.0 {
            continue;
        }
        let t = project_translation(scale(sub(center, *v), factor * strength), area_normal);
        *v = add(*v, add(t, scale(offset, factor)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: V3 = [0.0, 0.0, 1.0];

    #[test]
    fn strengths_follow_the_formulas() {
        assert_eq!(brush_strength(0.5, false), 0.25);
        assert_eq!(brush_strength(0.5, true), -0.25);
        /* |alpha²| * pinch² / alpha² = pinch². */
        assert!((pinch_strength(0.5, false, 0.5) - 0.25).abs() < 1e-6);
        assert!((pinch_strength(0.5, true, 0.5) - 0.25).abs() < 1e-6);
        assert_eq!(pinch_strength(0.0, false, 0.5), 0.0);
    }

    #[test]
    fn projection_removes_the_normal_component() {
        assert_eq!(project_translation([1.0, 2.0, 3.0], N), [1.0, 2.0, 0.0]);
        assert_eq!(project_translation([1.0, 2.0, 3.0], [0.0; 3]), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn pinches_sideways_and_digs_along_the_normal() {
        let mut v = vec![[0.5, 0.0, 0.0]];
        dab(&mut v, &[], N, [0.0; 3], 1.0, 1.0, false, 0.5, |_| 1.0);
        /* x: 0.5 + (-0.5 * 1 * 0.25) = 0.375; z: offset radius * alpha² = 1. */
        assert!((v[0][0] - 0.375).abs() < 1e-6);
        assert!((v[0][2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn inverted_offsets_the_other_way_but_still_pinches() {
        let mut v = vec![[0.5, 0.0, 0.0]];
        dab(&mut v, &[], N, [0.0; 3], 1.0, 1.0, true, 0.5, |_| 1.0);
        assert!((v[0][0] - 0.375).abs() < 1e-6);
        assert!((v[0][2] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn mask_and_radius_limit_the_effect() {
        let mut v = vec![[0.5, 0.0, 0.0], [0.5, 0.0, 0.0], [2.0, 0.0, 0.0]];
        dab(&mut v, &[1.0, 0.0, 0.0], N, [0.0; 3], 1.0, 1.0, false, 0.5, |_| 1.0);
        assert_eq!(v[0], [0.5, 0.0, 0.0]);
        assert_ne!(v[1], [0.5, 0.0, 0.0]);
        assert_eq!(v[2], [2.0, 0.0, 0.0]);
    }
}
