//! Curves sculpt Puff brush (editors/sculpt_paint/curves/sculpt_puff.cc): curves under the brush are aligned
//! to the surface normal at their root. Ports `find_curves_weights_spherical()` (:262-290) and `puff()`
//! (:293-345). Deviation: the closest surface point / normal and the falloff curve are given by the caller;
//! projected (tube) weights, symmetry and the constraint solver are not modelled.

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 { std::array::from_fn(|i| a[i] - b[i]) }
fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn len(a: V3) -> f32 { dot(a, a).sqrt() }

/// `dist_squared_to_line_segment_v3()`.
pub fn dist_sq_to_segment(p: V3, a: V3, b: V3) -> f32 {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    let t = if l2 == 0.0 { 0.0 } else { (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0) };
    let c: V3 = std::array::from_fn(|i| a[i] + ab[i] * t);
    let d = sub(p, c);
    dot(d, d)
}

/// Weight of one curve: the maximum falloff of its segments (the root point is skipped as a segment end like
/// `drop_front(1)`) within `radius` of the brush. `curve_strength(dist, radius)` is `BKE_brush_curve_strength()`.
pub fn curve_weight(points: &[V3], brush: V3, radius: f32, curve_strength: impl Fn(f32, f32) -> f32) -> f32 {
    let mut max_weight = 0.0f32;
    for w in points.windows(2) {
        let d2 = dist_sq_to_segment(brush, w[0], w[1]);
        if d2 > radius * radius {
            continue;
        }
        max_weight = max_weight.max(curve_strength(d2.sqrt(), radius));
    }
    max_weight
}

/// `puff()` for one curve: every point but the root moves toward `root + arc_length * normal` by
/// `0.01 * strength * point_factor * curve_weight`, and is pushed out so it is never closer to the root than before.
pub fn puff_curve(points: &[V3], normal: V3, strength: f32, point_factors: &[f32], curve_weight: f32) -> Vec<V3> {
    let Some(&first) = points.first() else { return Vec::new() };
    let mut out = points.to_vec();
    let mut accumulated = 0.0f32;
    for i in 1..points.len() {
        accumulated += len(sub(points[i], points[i - 1]));
        let old = points[i];
        let goal: V3 = std::array::from_fn(|k| first[k] + accumulated * normal[k]);
        let weight = 0.01 * strength * point_factors[i] * curve_weight;
        let mut new: V3 = std::array::from_fn(|k| old[k] + (goal[k] - old[k]) * weight);
        let old_dist = len(sub(old, first));
        let new_dist = len(sub(new, first));
        if new_dist < old_dist {
            let d = sub(new, first);
            let l = len(d);
            if l > 0.0 {
                for k in 0..3 {
                    new[k] += (old_dist - new_dist) * d[k] / l;
                }
            }
        }
        out[i] = new;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURVE: [V3; 3] = [[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];

    #[test]
    fn zero_strength_or_weight_leaves_points() {
        assert_eq!(puff_curve(&CURVE, [0.0, 0.0, 1.0], 0.0, &[1.0; 3], 1.0), CURVE.to_vec());
        assert_eq!(puff_curve(&CURVE, [0.0, 0.0, 1.0], 1.0, &[1.0; 3], 0.0), CURVE.to_vec());
    }

    #[test]
    fn root_is_unchanged_and_curve_rises_toward_the_normal_keeping_root_distance() {
        let out = puff_curve(&CURVE, [0.0, 0.0, 1.0], 100.0, &[1.0; 3], 1.0);
        assert_eq!(out[0], CURVE[0]);
        assert!(out[1][2] > 0.0 && out[2][2] > out[1][2]);
        for i in 1..3 {
            assert!((len(out[i]) - len(CURVE[i])).abs() < 1e-4 || len(out[i]) > len(CURVE[i]));
        }
    }

    #[test]
    fn falloff_is_zero_outside_the_radius_and_max_of_segments_inside() {
        let lin = |d: f32, r: f32| 1.0 - d / r;
        assert_eq!(curve_weight(&CURVE, [0.0, 5.0, 0.0], 1.0, lin), 0.0);
        let w = curve_weight(&CURVE, [1.0, 0.5, 0.0], 1.0, lin);
        assert!((w - 0.5).abs() < 1e-6);
        assert!((dist_sq_to_segment([3.0, 0.0, 0.0], CURVE[0], CURVE[1]) - 4.0).abs() < 1e-6);
    }
}
