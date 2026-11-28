//! Curves sculpt Snake Hook brush (editors/sculpt_paint/curves/sculpt_snake_hook.cc): the TIP (last point) of each
//! curve is dragged by the brush and the curve is resampled to the new length. Ports the per-curve weight of
//! `projected_snake_hook()` / `spherical_snake_hook()` (:165-196, :244-262) and `move_last_point_and_resample()`
//! (sculpt_brush.cc:354-390) with `length_parameterize::{accumulate_lengths, sample_at_length(s), interpolate}`
//! (BLI_length_parameterize.hh). Deviation: projection to the screen, symmetry and crazyspace translation are the caller's.

type V3 = [f32; 3];

fn dist(a: V3, b: V3) -> f32 { (0..3).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum::<f32>().sqrt() }

/// Weight of a curve whose tip is `distance` from the brush: `strength * falloff * curve_factor`; `None` when outside
/// `radius` (both snake hook variants skip when `distance^2 > radius^2`).
pub fn tip_weight(distance: f32, radius: f32, strength: f32, curve_factor: f32, curve_strength: impl Fn(f32, f32) -> f32) -> Option<f32> {
    if distance * distance > radius * radius { return None; }
    Some(strength * curve_strength(distance, radius) * curve_factor)
}

/// `sample_at_length()` without the hint: segment index and factor of `sample` in the accumulated `lengths`.
pub fn sample_at_length(lengths: &[f32], sample: f32) -> (usize, f32) {
    let total = *lengths.last().unwrap();
    if sample >= total { return (lengths.len() - 1, 1.0); }
    let prev = lengths.partition_point(|&l| l <= sample);
    let start = if prev == 0 { 0.0 } else { lengths[prev - 1] };
    let seg = lengths[prev] - start;
    let inv = if seg == 0.0 { 0.0 } else { 1.0 / seg };
    (prev, (sample - start) * inv)
}

/// `move_last_point_and_resample()`: put the last point at `new_last` and redistribute the others along the original
/// polyline scaled by the new/old total length.
pub fn move_last_point_and_resample(positions: &mut [V3], new_last: V3) {
    let n = positions.len();
    if n <= 2 {
        if let Some(l) = positions.last_mut() { *l = new_last; }
        return;
    }
    let mut orig = Vec::with_capacity(n - 1);
    let mut acc = 0.0;
    for i in 0..n - 1 { acc += dist(positions[i], positions[i + 1]); orig.push(acc); }
    let orig_total = orig[n - 2];
    let new_total = orig[n - 3] + dist(positions[n - 2], new_last);
    let factor = if orig_total == 0.0 { 0.0 } else { new_total / orig_total };
    let mut new_pos = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let target = if i == 0 { 0.0 } else { orig[i - 1] * factor };
        let (seg, f) = sample_at_length(&orig, target);
        let (a, b) = (positions[seg], positions[seg + 1]);
        new_pos.push(std::array::from_fn(|k| a[k] + (b[k] - a[k]) * f));
    }
    positions[..n - 1].copy_from_slice(&new_pos);
    positions[n - 1] = new_last;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_cut_and_scale() {
        let c = |d: f32, r: f32| 1.0 - d / r;
        assert_eq!(tip_weight(2.0, 1.0, 1.0, 1.0, c), None);
        assert!((tip_weight(0.5, 1.0, 0.8, 0.5, c).unwrap() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn two_point_curve_just_moves_tip() {
        let mut p = vec![[0.0; 3], [1.0, 0.0, 0.0]];
        move_last_point_and_resample(&mut p, [5.0, 5.0, 0.0]);
        assert_eq!(p, vec![[0.0; 3], [5.0, 5.0, 0.0]]);
    }

    #[test]
    fn stretch_samples_original_polyline_clamped_and_keeps_root() {
        // Original lengths are sampled at lengths scaled by new/old (2.0): 2 -> point 2, 4 -> clamped to the end.
        let mut p = vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
        move_last_point_and_resample(&mut p, [6.0, 0.0, 0.0]);
        assert_eq!(p[0], [0.0; 3]);
        assert_eq!(p[3], [6.0, 0.0, 0.0]);
        assert!((p[1][0] - 2.0).abs() < 1e-5 && (p[2][0] - 3.0).abs() < 1e-5);
    }

    #[test]
    fn shrink_scales_lengths_uniformly() {
        let mut p = vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
        move_last_point_and_resample(&mut p, [1.5, 0.0, 0.0]);
        let f = 2.5 / 3.0;
        assert!((p[1][0] - f).abs() < 1e-5 && (p[2][0] - 2.0 * f).abs() < 1e-5);
    }

    #[test]
    fn bent_curve_follows_original_polyline() {
        let mut p = vec![[0.0; 3], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0]];
        move_last_point_and_resample(&mut p, [2.0, 2.0, 0.0]);
        assert_eq!(p[1], [2.0, 0.0, 0.0]);
    }

    #[test]
    fn sample_clamps_at_end() {
        assert_eq!(sample_at_length(&[1.0, 2.0], 5.0), (1, 1.0));
        assert_eq!(sample_at_length(&[1.0, 2.0], 1.5), (1, 0.5));
    }
}
