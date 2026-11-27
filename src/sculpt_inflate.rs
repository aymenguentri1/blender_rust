//! Inflate brush (`mesh/brushes/inflate.cc`: `do_inflate_brush`; strength from `brush_strength()` in sculpt.cc). Each
//! vertex under the brush moves along its own normal by `radius * bstrength * factor`. Deviations: the object scale
//! (`StrokeCache::scale`) is 1 and the cube tip is not ported; overlap and pressure are 1.

use crate::mesh::{add, dot, scale, sub, V3};

/// `brush_strength()` for `SCULPT_BRUSH_TYPE_INFLATE`: `0.25 * alpha²`, half of that (and negative) when inverted.
pub fn brush_strength(alpha: f32, invert: bool) -> f32 {
    let a = alpha * alpha;
    if invert {
        -0.125 * a
    } else {
        0.25 * a
    }
}

/// One dab over the vertices `area` (index, normal). `falloff` maps the distance in brush radii (0..1) to the factor.
#[allow(clippy::too_many_arguments)]
pub fn dab(verts: &mut [V3], mask: &[f32], area: &[(usize, V3)], center: V3, radius: f32, bstrength: f32, falloff: impl Fn(f32) -> f32) {
    let amount = radius * bstrength;
    for &(i, n) in area {
        let d = sub(verts[i], center);
        let dist = dot(d, d).sqrt();
        if dist >= radius {
            continue;
        }
        let m = (1.0 - mask.get(i).copied().unwrap_or(0.0)).clamp(0.0, 1.0);
        let factor = falloff(dist / radius) * m;
        verts[i] = add(verts[i], scale(n, amount * factor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_quarter_alpha_squared_and_half_when_inverted() {
        assert_eq!(brush_strength(0.5, false), 0.0625);
        assert_eq!(brush_strength(0.5, true), -0.03125);
    }

    #[test]
    fn moves_along_each_normal_by_radius_strength_factor() {
        let mut v = vec![[0.0, 0.0, 0.0], [0.5, 0.0, 0.0]];
        let area = [(0, [0.0, 0.0, 1.0]), (1, [1.0, 0.0, 0.0])];
        dab(&mut v, &[], &area, [0.0; 3], 2.0, 0.5, |d| 1.0 - d);
        assert_eq!(v[0], [0.0, 0.0, 1.0]); /* 2 * 0.5 * 1.0 */
        assert!((v[1][0] - (0.5 + 2.0 * 0.5 * 0.75)).abs() < 1e-6);
    }

    #[test]
    fn mask_scales_and_outside_is_untouched() {
        let mut v = vec![[0.0; 3], [0.0; 3], [3.0, 0.0, 0.0]];
        let n = [0.0, 0.0, 1.0];
        dab(&mut v, &[0.0, 0.75, 0.0], &[(0, n), (1, n), (2, n)], [0.0; 3], 1.0, 1.0, |_| 1.0);
        assert_eq!(v[0][2], 1.0);
        assert_eq!(v[1][2], 0.25);
        assert_eq!(v[2], [3.0, 0.0, 0.0]);
    }

    #[test]
    fn inverted_deflates() {
        let mut v = vec![[0.0; 3]];
        dab(&mut v, &[], &[(0, [0.0, 0.0, 1.0])], [0.0; 3], 1.0, brush_strength(1.0, true), |_| 1.0);
        assert!(v[0][2] < 0.0);
    }
}
