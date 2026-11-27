//! Snake Hook brush, standard (non-elastic) mode (`mesh/brushes/snake_hook.cc`: `calc_pinch_influence`, `calc_faces`,
//! `do_snake_hook_brush`; strength from `brush_strength()` in sculpt.cc, which is the plain alpha). Every vertex under
//! the brush moves by the stroke delta times its factor (`falloff * (1 - mask) * bstrength`); a `crease_pinch_factor`
//! other than 0.5 additionally pinches the vertices toward the hook line.
//! Deviations: the Elastic (Kelvinlet) deform type, rake rotation and the Tube falloff are not ported.

use crate::mesh::{add, dot, scale, sub, V3};

/// The per-vertex translations of one dab at `center` for the stroke `delta`. Returns `(vertex, translation)` for
/// every vertex with a non-zero factor. `falloff` maps the distance in brush radii (0..1) to the factor.
#[allow(clippy::too_many_arguments)]
pub fn translations(verts: &[V3], mask: &[f32], center: V3, radius: f32, bstrength: f32, delta: V3, pinch_factor: f32, falloff: impl Fn(f32) -> f32) -> Vec<(usize, V3)> {
    let delta_len = dot(delta, delta).sqrt();
    let len_sq = dot(delta, delta);
    let valid = len_sq > f32::EPSILON;
    let pinch = if pinch_factor == 0.5 { 0.0 } else { 2.0 * (0.5 - pinch_factor) * delta_len / radius };
    let mut out = Vec::new();
    for (i, &v) in verts.iter().enumerate() {
        let d = sub(v, center);
        let dist = dot(d, d).sqrt();
        if dist >= radius {
            continue;
        }
        let factor = falloff(dist / radius) * (1.0 - mask.get(i).copied().unwrap_or(0.0)).clamp(0.0, 1.0) * bstrength;
        if factor == 0.0 {
            continue;
        }
        let mut t = scale(delta, factor);
        if pinch_factor != 0.5 {
            /* The grabbed location, projected onto the plane perpendicular to the stroke. */
            let mut dp = add(d, delta);
            if valid {
                dp = add(dp, scale(delta, dot(dp, delta) * (-1.0 / len_sq)));
            }
            let init = dp;
            let mut fade = pinch * factor;
            if pinch > 0.0 {
                let c = (dot(dp, dp).sqrt() / radius).min(1.0);
                fade *= c * c;
            }
            dp = scale(dp, 1.0 + fade);
            t = add(t, sub(init, dp));
        }
        out.push((i, t));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELTA: V3 = [1.0, 0.0, 0.0];

    fn run(verts: &[V3], mask: &[f32], bstrength: f32, pinch: f32) -> Vec<(usize, V3)> {
        translations(verts, mask, [0.0; 3], 2.0, bstrength, DELTA, pinch, |d| 1.0 - d)
    }

    #[test]
    fn moves_by_delta_times_falloff_and_strength() {
        let t = run(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], &[], 0.5, 0.5);
        assert_eq!(t[0], (0, [0.5, 0.0, 0.0]));
        assert_eq!(t[1], (1, [0.25, 0.0, 0.0]));
    }

    #[test]
    fn mask_zeroes_and_radius_limits() {
        let t = run(&[[0.0; 3], [0.0; 3], [3.0, 0.0, 0.0]], &[1.0, 0.0, 0.0], 1.0, 0.5);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].0, 1);
    }

    #[test]
    fn default_pinch_factor_does_not_pinch() {
        let t = run(&[[0.0, 0.5, 0.0]], &[], 1.0, 0.5);
        assert_eq!(t[0].1[1], 0.0);
    }

    #[test]
    fn lower_pinch_factor_pulls_toward_the_hook_line_and_higher_inflates() {
        /* pinch = 2 * (0.5 - 0.0) * 1 / 2 = 0.5 > 0: the offset 0.5 from the line shrinks. */
        let t = run(&[[0.0, 0.5, 0.0]], &[], 1.0, 0.0);
        assert!(t[0].1[1] < 0.0);
        let t = run(&[[0.0, 0.5, 0.0]], &[], 1.0, 1.0);
        assert!(t[0].1[1] > 0.0);
    }

    #[test]
    fn pinch_leaves_the_stroke_axis_alone() {
        let t = run(&[[0.0, 0.5, 0.0]], &[], 1.0, 0.0);
        /* x only gets the delta term: factor 0.75. */
        assert!((t[0].1[0] - 0.75).abs() < 1e-6);
    }
}
