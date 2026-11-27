//! Layer brush (`mesh/brushes/layer.cc`: `offset_displacement_factors`, `clamp_displacement_factors`, `calc_translations`,
//! `calc_faces` without the persistent base; strength from `brush_strength()` in sculpt.cc). Each vertex keeps a
//! displacement factor for the stroke; a dab raises it by `factor * bstrength * (1.05 - |d|)`, clamps it to
//! `+-(1 - mask)` and moves the vertex a `factor` fraction of the way to `orig + orig_normal * height * d`.
//! Falloff distances are measured on the stroke start (original) positions.
//! Deviations: the persistent base and its reset mode are not ported; `Brush::height` is 0 in the DNA and set by the
//! Layer brush asset, which is not in the reference repository, so `HEIGHT` is an assumption (Blender's `0.05`).

use crate::mesh::{add, dot, scale, sub, V3};

/// Assumed `Brush::height` of the Layer brush.
pub const HEIGHT: f32 = 0.05;

/// `brush_strength()` for `SCULPT_BRUSH_TYPE_LAYER`: `alpha² * flip` (overlap and pressure 1).
pub fn brush_strength(alpha: f32, invert: bool) -> f32 {
    alpha * alpha * if invert { -1.0 } else { 1.0 }
}

/// `offset_displacement_factors()` + `clamp_displacement_factors()` for one vertex.
pub fn next_displacement(d: f32, factor: f32, bstrength: f32, mask: f32) -> f32 {
    let d = d + factor * bstrength * (1.05 - d.abs());
    let limit = 1.0 - mask;
    d.clamp(-limit, limit)
}

/// One dab over all vertices. `orig`/`normals` are the stroke start positions and normals, `disp` the per-vertex
/// displacement factors kept for the stroke. `falloff` maps the distance in brush radii (0..1) to the factor.
#[allow(clippy::too_many_arguments)]
pub fn dab(verts: &mut [V3], orig: &[V3], normals: &[V3], disp: &mut [f32], mask: &[f32], center: V3, radius: f32, bstrength: f32, height: f32, falloff: impl Fn(f32) -> f32) {
    for i in 0..verts.len() {
        let d = sub(orig[i], center);
        let dist = dot(d, d).sqrt();
        if dist >= radius {
            continue;
        }
        let m = mask.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let factor = falloff(dist / radius) * (1.0 - m);
        if factor <= 0.0 {
            continue;
        }
        disp[i] = next_displacement(disp[i], factor, bstrength, m);
        let target = add(orig[i], scale(normals[i], height * disp[i]));
        verts[i] = add(verts[i], scale(sub(target, verts[i]), factor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: V3 = [0.0, 0.0, 1.0];

    #[test]
    fn displacement_grows_slower_near_the_top_and_clamps() {
        assert!((next_displacement(0.0, 1.0, 0.5, 0.0) - 0.525).abs() < 1e-6);
        assert!((next_displacement(1.0, 1.0, 0.5, 0.0) - 1.0).abs() < 1e-6);
        assert_eq!(next_displacement(0.9, 1.0, 1.0, 0.0), 1.0);
        assert_eq!(next_displacement(-0.9, 1.0, -1.0, 0.0), -1.0);
        /* A half masked vertex is clamped to 0.5. */
        assert_eq!(next_displacement(0.4, 1.0, 1.0, 0.5), 0.5);
    }

    #[test]
    fn strength_is_alpha_squared_signed() {
        assert_eq!(brush_strength(0.5, false), 0.25);
        assert_eq!(brush_strength(0.5, true), -0.25);
    }

    #[test]
    fn plateau_reaches_the_height_and_stays() {
        let orig = vec![[0.0; 3]];
        let mut v = orig.clone();
        let mut disp = vec![0.0];
        for _ in 0..50 {
            dab(&mut v, &orig, &[N], &mut disp, &[], [0.0; 3], 1.0, 1.0, 0.1, |_| 1.0);
        }
        assert!((v[0][2] - 0.1).abs() < 1e-6);
        assert_eq!(disp[0], 1.0);
    }

    #[test]
    fn partial_factor_moves_a_fraction_of_the_way() {
        let orig = vec![[0.5, 0.0, 0.0]];
        let mut v = orig.clone();
        let mut disp = vec![0.0];
        dab(&mut v, &orig, &[N], &mut disp, &[], [0.0; 3], 1.0, 1.0, 1.0, |d| 1.0 - d);
        /* factor 0.5, disp 0.5 * 1.05, target z 0.525, moved half of it. */
        assert!((disp[0] - 0.525).abs() < 1e-6);
        assert!((v[0][2] - 0.2625).abs() < 1e-6);
    }

    #[test]
    fn inverted_digs_and_mask_blocks() {
        let orig = vec![[0.0; 3], [0.0; 3], [5.0, 0.0, 0.0]];
        let mut v = orig.clone();
        let mut disp = vec![0.0; 3];
        dab(&mut v, &orig, &[N, N, N], &mut disp, &[0.0, 1.0, 0.0], [0.0; 3], 1.0, -1.0, 0.1, |_| 1.0);
        assert!(v[0][2] < 0.0);
        assert_eq!(v[1], [0.0; 3]);
        assert_eq!(v[2], [5.0, 0.0, 0.0]);
    }
}
