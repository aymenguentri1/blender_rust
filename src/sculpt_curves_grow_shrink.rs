//! Curves sculpt Grow/Shrink brush (editors/sculpt_paint/curves/sculpt_grow_shrink.cc): curves change length by
//! the brush weight times a move distance. Ports `ShrinkCurvesEffect::shrink_curve()` (:107-132),
//! `ScaleCurvesEffect` (:186-216) and the brush `weight` (:397-399). Deviation: the extrapolate effect (move the
//! last point and resample) is not modelled; the move distance and falloff curve come from the caller.

type V3 = [f32; 3];

fn dist(a: V3, b: V3) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

/// `compute_poly_curve_length()`.
pub fn poly_length(points: &[V3]) -> f32 {
    points.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// `weight = brush_strength * radius_falloff * curve_selection_factor`.
pub fn brush_weight(strength: f32, radius_falloff: f32, selection_factor: f32) -> f32 {
    strength * radius_falloff * selection_factor
}

/// `ScaleCurvesEffect::execute()`: `new_length = max(minimum_length, old_length +- move_distance)`, every point
/// but the root is scaled about the root by `safe_divide(new_length, old_length)`.
pub fn scale_curve(points: &mut [V3], move_distance: f32, scale_up: bool, minimum_length: f32) {
    if points.is_empty() {
        return;
    }
    let old_length = poly_length(points);
    let diff = if scale_up { move_distance } else { -move_distance };
    let new_length = minimum_length.max(old_length + diff);
    let factor = if old_length == 0.0 { 0.0 } else { new_length / old_length };
    let root = points[0];
    for p in &mut points[1..] {
        *p = std::array::from_fn(|k| (p[k] - root[k]) * factor + root[k]);
    }
}

/// `ShrinkCurvesEffect::shrink_curve()`: the points are resampled at `old_arc_length * length_factor` along the
/// old polyline with `length_factor = clamp(max(min, old - shrink) / old, 0, 1)`.
pub fn shrink_curve(points: &mut [V3], shrink_length: f32, minimum_length: f32) {
    if points.len() < 2 {
        return;
    }
    let old = points.to_vec();
    let mut acc = vec![0.0f32];
    for w in old.windows(2) {
        acc.push(acc.last().unwrap() + dist(w[0], w[1]));
    }
    let old_length = *acc.last().unwrap();
    let new_length = minimum_length.max(old_length - shrink_length);
    let factor = if old_length == 0.0 { 0.0 } else { (new_length / old_length).clamp(0.0, 1.0) };
    for i in 1..old.len() {
        let s = acc[i] * factor;
        /* Segment containing `s`. */
        let seg = (1..acc.len()).find(|&j| acc[j] >= s).unwrap_or(acc.len() - 1);
        let (l0, l1) = (acc[seg - 1], acc[seg]);
        let t = if l1 > l0 { (s - l0) / (l1 - l0) } else { 0.0 };
        points[i] = std::array::from_fn(|k| old[seg - 1][k] + (old[seg][k] - old[seg - 1][k]) * t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> Vec<V3> {
        vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]
    }

    #[test]
    fn grow_lengthens_shrink_shortens_and_zero_leaves() {
        let mut g = line();
        scale_curve(&mut g, 1.0, true, 0.0);
        assert!((poly_length(&g) - 3.0).abs() < 1e-5);
        let mut s = line();
        scale_curve(&mut s, 1.0, false, 0.0);
        assert!((poly_length(&s) - 1.0).abs() < 1e-5);
        let mut z = line();
        scale_curve(&mut z, 0.0, true, 0.0);
        assert_eq!(z, line());
        assert_eq!(g[0], [0.0; 3]);
    }

    #[test]
    fn minimum_length_is_respected() {
        let mut s = line();
        scale_curve(&mut s, 10.0, false, 0.5);
        assert!((poly_length(&s) - 0.5).abs() < 1e-5);
        let mut r = line();
        shrink_curve(&mut r, 10.0, 0.5);
        assert!((poly_length(&r) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn shrink_resamples_along_the_old_curve_and_keeps_root() {
        let mut r = line();
        shrink_curve(&mut r, 1.0, 0.0);
        assert_eq!(r[0], [0.0; 3]);
        assert!((r[1][0] - 0.5).abs() < 1e-5 && (r[2][0] - 1.0).abs() < 1e-5);
        let mut n = line();
        shrink_curve(&mut n, 0.0, 0.0);
        assert_eq!(n, line());
        assert_eq!(brush_weight(2.0, 0.5, 0.5), 0.5);
    }
}
