//! Curves sculpt Smooth brush (editors/sculpt_paint/curves/sculpt_smooth.cc): points move toward the midpoint of
//! their neighbours. Ports the per-point weight of `find_projected_smooth_factors()` / `find_spherical_smooth_factors()`
//! (:150-165, :208-222) and `smooth()` (:226-250). Deviation: distances are given by the caller (screen or 3D),
//! the falloff curve is a closure (`BKE_brush_curve_strength()`), symmetry copies are max-combined by the caller.

type V3 = [f32; 3];

/// Used to make the brush easier to use ("a strength of 1 would be way too large").
pub const WEIGHT_FACTOR: f32 = 0.1;

/// Smooth factor of one point at `dist` from the brush, or `None` outside `radius` (`dist^2 > radius^2` skips).
pub fn point_weight(dist: f32, radius: f32, strength: f32, point_factor: f32, curve_strength: impl Fn(f32, f32) -> f32) -> Option<f32> {
    if dist * dist > radius * radius { return None; }
    Some(WEIGHT_FACTOR * strength * curve_strength(dist, radius) * point_factor)
}

/// `math::max_inplace(r_point_smooth_factors[i], weight)`.
pub fn accumulate(factors: &mut [f32], point: usize, weight: f32) {
    factors[point] = factors[point].max(weight);
}

/// `smooth()` for one curve: every point but the first and last moves toward the midpoint of its OLD neighbours
/// by its factor; factor 0 skips the point.
pub fn smooth_curve(points: &mut [V3], factors: &[f32]) {
    if points.len() < 3 { return; }
    let old = points.to_vec();
    for i in 1..old.len() - 1 {
        let f = factors[i];
        if f == 0.0 { continue; }
        for k in 0..3 {
            let goal = (old[i - 1][k] + old[i + 1][k]) * 0.5;
            points[i][k] = old[i][k] + (goal - old[i][k]) * f;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_applies_factor_and_radius_cut() {
        let lin = |d: f32, r: f32| 1.0 - d / r;
        assert_eq!(point_weight(2.0, 1.0, 1.0, 1.0, lin), None);
        let w = point_weight(0.5, 1.0, 1.0, 1.0, lin).unwrap();
        assert!((w - 0.05).abs() < 1e-6);
        assert_eq!(point_weight(0.0, 1.0, 1.0, 0.0, lin), Some(0.0));
    }

    #[test]
    fn accumulate_takes_max() {
        let mut f = [0.0, 0.3];
        accumulate(&mut f, 1, 0.1);
        accumulate(&mut f, 0, 0.2);
        assert_eq!(f, [0.2, 0.3]);
    }

    #[test]
    fn zero_factor_is_noop_and_ends_are_fixed() {
        let orig = vec![[0.0, 0.0, 0.0], [1.0, 5.0, 0.0], [2.0, 0.0, 0.0]];
        let mut p = orig.clone();
        smooth_curve(&mut p, &[0.0; 3]);
        assert_eq!(p, orig);
        smooth_curve(&mut p, &[1.0; 3]);
        assert_eq!(p[0], orig[0]);
        assert_eq!(p[2], orig[2]);
        assert_eq!(p[1], [1.0, 0.0, 0.0]);
    }

    #[test]
    fn smoothing_uses_old_neighbours() {
        let mut p = vec![[0.0; 3], [0.0, 10.0, 0.0], [0.0; 3], [0.0; 3]];
        smooth_curve(&mut p, &[0.0, 1.0, 1.0, 0.0]);
        assert_eq!(p[1][1], 0.0);
        assert_eq!(p[2][1], 5.0);
    }
}
