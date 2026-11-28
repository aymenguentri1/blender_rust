//! Curves sculpt Comb brush (editors/sculpt_paint/curves/sculpt_comb.cc). Ports the per-point weight of
//! `comb_projected()` / `comb_spherical()` and the resulting translation: every point except the root moves by
//! the brush delta scaled by `strength * curve_falloff(length_param) * radius_falloff(dist) * point_factor`.
//! Deviation: the caller supplies distances (screen or 3D), projects/unprojects and runs the constraint solver.

#![allow(dead_code)]

pub use crate::sculpt_curves_snake_hook::dist_sq_to_segment_v3;

type V2 = [f32; 2];
type V3 = [f32; 3];

/// `dist_squared_to_line_segment_v2()`.
pub fn dist_sq_to_segment_v2(p: V2, a: V2, b: V2) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let l2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if l2 == 0.0 { 0.0 } else { (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2).clamp(0.0, 1.0) };
    let d = [p[0] - (a[0] + ab[0] * t), p[1] - (a[1] + ab[1] * t)];
    d[0] * d[0] + d[1] * d[1]
}

/// `math::safe_rcp()`: 0 for 0.
pub fn safe_rcp(x: f32) -> f32 {
    if x != 0.0 { 1.0 / x } else { 0.0 }
}

/// Total curve length: the sum of its segment lengths (`curve_lengths_` in the stroke operation).
pub fn curve_length(segment_lengths: &[f32]) -> f32 {
    segment_lengths.iter().sum()
}

/// Weight per point of one curve. `dist_sq[i]` is the squared distance of point `i` to the brush (screen px or
/// 3D units, matching `radius`); `segment_lengths[i]` joins points `i` and `i + 1`. The root (point 0) and points
/// outside the radius give `None`; others `Some(strength * curve_falloff(param) * radius_falloff(dist, radius) *
/// point_factors[i])` where `param` is the length from the root divided by the total length.
pub fn comb_point_weights(
    segment_lengths: &[f32],
    dist_sq: &[f32],
    radius: f32,
    strength: f32,
    point_factors: &[f32],
    radius_falloff: impl Fn(f32, f32) -> f32,
    curve_falloff: impl Fn(f32) -> f32,
) -> Vec<Option<f32>> {
    let total_inv = safe_rcp(curve_length(segment_lengths));
    let radius_sq = radius * radius;
    let mut current = 0.0;
    let mut out = vec![None; dist_sq.len()];
    for i in 1..dist_sq.len() {
        current += segment_lengths[i - 1];
        if dist_sq[i] > radius_sq {
            continue;
        }
        let rf = radius_falloff(dist_sq[i].sqrt(), radius);
        let cf = curve_falloff(current * total_inv);
        out[i] = Some(strength * cf * rf * point_factors[i]);
    }
    out
}

/// Projected variant: the new screen position of a point.
pub fn projected_new_pos(old_re: V2, brush_diff_re: V2, weight: f32) -> V2 {
    [old_re[0] + brush_diff_re[0] * weight, old_re[1] + brush_diff_re[1] * weight]
}

/// Spherical variant: translation `weight * (brush_end - brush_start)`.
pub fn spherical_translation(brush_diff: V3, weight: f32) -> V3 {
    [brush_diff[0] * weight, brush_diff[1] * weight, brush_diff[2] * weight]
}

#[cfg(test)]
mod tests {
    use super::*;
    const LIN: fn(f32, f32) -> f32 = |d, r| 1.0 - d / r;
    const FLAT: fn(f32) -> f32 = |_| 1.0;

    #[test]
    fn zero_delta_is_a_noop() {
        assert_eq!(projected_new_pos([3.0, 4.0], [0.0, 0.0], 0.7), [3.0, 4.0]);
        assert_eq!(spherical_translation([0.0; 3], 0.7), [0.0; 3]);
    }

    #[test]
    fn root_and_outside_points_do_not_move() {
        let w = comb_point_weights(&[1.0, 1.0, 1.0], &[0.0, 0.25, 4.0, 0.0], 1.0, 1.0, &[1.0; 4], LIN, FLAT);
        assert_eq!(w[0], None); // root fixed even at distance 0
        assert_eq!(w[2], None); // outside radius
        assert!((w[1].unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(w[3], Some(1.0));
    }

    #[test]
    fn weight_combines_strength_curve_falloff_and_point_factor() {
        // lengths 1,3: params 0.25 and 1.0; curve falloff = param
        let w = comb_point_weights(&[1.0, 3.0], &[0.0, 0.0, 0.0], 2.0, 0.5, &[1.0, 1.0, 0.5], LIN, |t| t);
        assert!((w[1].unwrap() - 0.5 * 0.25).abs() < 1e-6);
        assert!((w[2].unwrap() - 0.5 * 1.0 * 0.5).abs() < 1e-6);
    }

    #[test]
    fn zero_length_curve_uses_zero_param() {
        let w = comb_point_weights(&[0.0], &[0.0, 0.0], 1.0, 1.0, &[1.0; 2], LIN, |t| 1.0 - t);
        assert_eq!(w[1], Some(1.0));
    }

    #[test]
    fn movement_scales_with_weight() {
        assert_eq!(projected_new_pos([1.0, 1.0], [10.0, -4.0], 0.5), [6.0, -1.0]);
        assert_eq!(spherical_translation([2.0, 0.0, 4.0], 0.25), [0.5, 0.0, 1.0]);
    }

    #[test]
    fn segment_distance_v2() {
        assert_eq!(dist_sq_to_segment_v2([1.0, 2.0], [0.0, 0.0], [2.0, 0.0]), 4.0);
        assert_eq!(dist_sq_to_segment_v2([5.0, 0.0], [0.0, 0.0], [2.0, 0.0]), 9.0);
        assert_eq!(dist_sq_to_segment_v2([1.0, 1.0], [0.0, 0.0], [0.0, 0.0]), 2.0);
    }
}
