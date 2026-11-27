//! Curves sculpt Pinch brush (editors/sculpt_paint/curves/sculpt_pinch.cc): points of the brush radius move
//! toward the brush centre by `invert * 0.1 * strength * t * curve(t) * point_factor`, the curve root (first
//! point) never moves. Ports the weight maths of `pinch_projected()` / `pinch_spherical()` (:177-203, :290-305)
//! and `new_pinch_operation()` (:318-322). Deviation: the falloff curve `BKE_brush_curve_strength()` is a closure
//! given by the caller; projection, symmetry and crazyspace deformation are not modelled.

/// `invert_pinch = (mode == Invert) != ((brush.flag & BRUSH_DIR_IN) != 0)`.
pub fn invert_pinch(stroke_invert: bool, brush_dir_in: bool) -> bool {
    stroke_invert != brush_dir_in
}

/// Pinch weight of a point at `dist` from the brush centre, `None` outside the radius (`dist^2 > radius^2`).
/// `radius_base` is the unscaled brush radius `t` is relative to, `radius` the factor-scaled one.
pub fn pinch_weight(
    dist: f32,
    radius_base: f32,
    radius: f32,
    strength: f32,
    invert: bool,
    point_factor: f32,
    curve_strength: impl Fn(f32) -> f32,
) -> Option<f32> {
    if dist * dist > radius * radius {
        return None;
    }
    let t = if radius_base == 0.0 { 0.0 } else { dist / radius_base };
    let falloff = t * curve_strength(t);
    let invert_factor = if invert { -1.0 } else { 1.0 };
    Some(invert_factor * 0.1 * strength * falloff * point_factor)
}

/// `math::interpolate(old, centre, weight)` for every point but the root, which is skipped like
/// `points.drop_front(1)`. `point_factors` has one entry per point; returns the new positions.
pub fn pinch_curve(
    points: &[[f32; 3]],
    center: [f32; 3],
    radius_base: f32,
    radius: f32,
    strength: f32,
    invert: bool,
    point_factors: &[f32],
    curve_strength: impl Fn(f32) -> f32 + Copy,
) -> Vec<[f32; 3]> {
    points
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            if i == 0 {
                return p;
            }
            let dist = (0..3).map(|k| (p[k] - center[k]).powi(2)).sum::<f32>().sqrt();
            match pinch_weight(dist, radius_base, radius, strength, invert, point_factors[i], curve_strength) {
                Some(w) => std::array::from_fn(|k| p[k] + (center[k] - p[k]) * w),
                None => p,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear(_t: f32) -> f32 {
        1.0
    }

    #[test]
    fn weight_is_t_times_curve_scaled_by_tenth_of_strength() {
        let w = pinch_weight(0.5, 1.0, 1.0, 2.0, false, 1.0, linear).unwrap();
        assert!((w - 0.1).abs() < 1e-6);
        assert_eq!(pinch_weight(0.0, 1.0, 1.0, 2.0, false, 1.0, linear), Some(0.0));
    }

    #[test]
    fn outside_the_radius_does_not_move_and_invert_flips() {
        assert_eq!(pinch_weight(1.5, 1.0, 1.0, 1.0, false, 1.0, linear), None);
        let a = pinch_weight(0.5, 1.0, 1.0, 1.0, false, 1.0, linear).unwrap();
        let b = pinch_weight(0.5, 1.0, 1.0, 1.0, true, 1.0, linear).unwrap();
        assert_eq!(a, -b);
        assert!(invert_pinch(true, false) && invert_pinch(false, true));
        assert!(!invert_pinch(true, true) && !invert_pinch(false, false));
    }

    #[test]
    fn curve_moves_toward_centre_but_keeps_the_root() {
        let pts = [[1.0, 0.0, 0.0], [0.5, 0.0, 0.0], [3.0, 0.0, 0.0]];
        let out = pinch_curve(&pts, [0.0; 3], 1.0, 1.0, 10.0, false, &[1.0; 3], linear);
        assert_eq!(out[0], pts[0]);
        assert!(out[1][0] < 0.5 && out[1][0] > 0.0);
        assert_eq!(out[2], pts[2]);
        let zero = pinch_curve(&pts, [0.0; 3], 1.0, 1.0, 10.0, false, &[0.0; 3], linear);
        assert_eq!(zero, pts.to_vec());
    }
}
