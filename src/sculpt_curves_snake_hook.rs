//! Curves sculpt Snake Hook brush (editors/sculpt_paint/curves/sculpt_snake_hook.cc): the last point of each
//! curve follows the brush motion. Ports the per-curve weight and last-point translation of
//! `projected_snake_hook()` (screen space, distance to the previous brush position) and `spherical_snake_hook()`
//! (distance to the brush's start-end segment, translation `weight * (end - start)`). The first stroke
//! sample only records the position. Deviation: the caller projects/unprojects and runs
//! `move_last_point_and_resample()` (not in this file); symmetry copies are separate calls.

type V2 = [f32; 2];
type V3 = [f32; 3];

/// `dist_squared_to_line_segment_v3()`.
pub fn dist_sq_to_segment_v3(p: V3, a: V3, b: V3) -> f32 {
    let ab: V3 = std::array::from_fn(|i| b[i] - a[i]);
    let l2: f32 = ab.iter().map(|v| v * v).sum();
    let t = if l2 == 0.0 { 0.0 } else { ((0..3).map(|i| (p[i] - a[i]) * ab[i]).sum::<f32>() / l2).clamp(0.0, 1.0) };
    (0..3).map(|i| { let d = p[i] - (a[i] + ab[i] * t); d * d }).sum()
}

/// `brush_strength * falloff(dist, radius) * curve_factor`, 0 when `dist_sq > radius^2`.
pub fn hook_weight(dist_sq: f32, radius: f32, strength: f32, curve_factor: f32, falloff: impl Fn(f32, f32) -> f32) -> f32 {
    if dist_sq > radius * radius {
        return 0.0;
    }
    strength * falloff(dist_sq.sqrt(), radius) * curve_factor
}

/// Projected variant: weight from the curve tip's screen position `tip_re` vs the previous brush position, and
/// the new screen position `tip_re + (brush - prev_brush) * weight` the caller unprojects. `None` outside the radius.
pub fn projected_new_tip(tip_re: V2, brush_prev_re: V2, brush_re: V2, radius: f32, strength: f32, curve_factor: f32, falloff: impl Fn(f32, f32) -> f32) -> Option<V2> {
    let d = [tip_re[0] - brush_prev_re[0], tip_re[1] - brush_prev_re[1]];
    let dist_sq = d[0] * d[0] + d[1] * d[1];
    if dist_sq > radius * radius {
        return None;
    }
    let w = hook_weight(dist_sq, radius, strength, curve_factor, falloff);
    Some([tip_re[0] + (brush_re[0] - brush_prev_re[0]) * w, tip_re[1] + (brush_re[1] - brush_prev_re[1]) * w])
}

/// Spherical variant: translation of the curve tip, `None` outside the radius of the start-end segment.
pub fn spherical_translation(tip: V3, start: V3, end: V3, radius: f32, strength: f32, curve_factor: f32, falloff: impl Fn(f32, f32) -> f32) -> Option<V3> {
    let dist_sq = dist_sq_to_segment_v3(tip, start, end);
    if dist_sq > radius * radius {
        return None;
    }
    let w = hook_weight(dist_sq, radius, strength, curve_factor, falloff);
    Some(std::array::from_fn(|i| w * (end[i] - start[i])))
}

#[cfg(test)]
mod tests {
    use super::*;
    const LIN: fn(f32, f32) -> f32 = |d, r| 1.0 - d / r;

    #[test]
    fn weight_is_strength_falloff_factor_and_zero_outside() {
        assert!((hook_weight(0.25, 1.0, 0.8, 0.5, LIN) - 0.8 * 0.5 * 0.5).abs() < 1e-6);
        assert_eq!(hook_weight(1.5, 1.0, 1.0, 1.0, LIN), 0.0);
    }

    #[test]
    fn projected_moves_tip_by_brush_motion_times_weight() {
        let p = projected_new_tip([0.0, 0.0], [0.0, 0.0], [10.0, 4.0], 2.0, 1.0, 1.0, |_, _| 0.5).unwrap();
        assert_eq!(p, [5.0, 2.0]);
    }

    #[test]
    fn projected_uses_previous_brush_position_for_distance() {
        assert!(projected_new_tip([5.0, 0.0], [0.0, 0.0], [5.0, 0.0], 2.0, 1.0, 1.0, LIN).is_none());
        assert!(projected_new_tip([5.0, 0.0], [5.0, 0.0], [9.0, 0.0], 2.0, 1.0, 1.0, LIN).is_some());
    }

    #[test]
    fn spherical_translates_along_the_brush_diff() {
        let t = spherical_translation([1.0, 0.0, 0.0], [0.0; 3], [2.0, 0.0, 0.0], 1.0, 1.0, 1.0, |_, _| 0.5).unwrap();
        assert_eq!(t, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn spherical_outside_segment_radius_is_skipped_and_curve_factor_scales() {
        assert!(spherical_translation([1.0, 3.0, 0.0], [0.0; 3], [2.0, 0.0, 0.0], 1.0, 1.0, 1.0, LIN).is_none());
        let t = spherical_translation([1.0, 0.0, 0.0], [0.0; 3], [2.0, 0.0, 0.0], 1.0, 1.0, 0.5, |_, _| 1.0).unwrap();
        assert_eq!(t, [1.0, 0.0, 0.0]);
        assert!((dist_sq_to_segment_v3([5.0, 0.0, 0.0], [0.0; 3], [2.0, 0.0, 0.0]) - 9.0).abs() < 1e-6);
    }
}
