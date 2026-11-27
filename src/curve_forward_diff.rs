//! `BKE_curve_forward_diff_bezier()` and `BKE_curve_forward_diff_tangent_bezier()` (curve.cc):
//! one axis of a cubic Bezier segment sampled at `it + 1` equal parameter steps. The C functions
//! write through a stride; here the values are returned and the caller interleaves the axes.

/// Positions at `t = i / it` of the segment with control values `q`. `it` must be at least 1.
pub fn forward_diff_bezier(q: [f32; 4], it: usize) -> Vec<f32> {
    let [q0, q1, q2, q3] = q;
    let mut f = it as f32;
    let rt1 = 3.0 * (q1 - q0) / f;
    f *= f;
    let rt2 = 3.0 * (q0 - 2.0 * q1 + q2) / f;
    f *= it as f32;
    let rt3 = (q3 - q0 + 3.0 * (q1 - q2)) / f;

    let (mut p, mut d1, mut d2, d3) = (q0, rt1 + rt2 + rt3, 2.0 * rt2 + 6.0 * rt3, 6.0 * rt3);
    (0..=it)
        .map(|_| {
            let out = p;
            p += d1;
            d1 += d2;
            d2 += d3;
            out
        })
        .collect()
}

/// Derivatives (with respect to `t`) at `t = i / it`. `it` must be at least 1.
pub fn forward_diff_tangent_bezier(q: [f32; 4], it: usize) -> Vec<f32> {
    let [q0, q1, q2, q3] = q;
    let f = 1.0 / it as f32;

    let rt0 = 3.0 * (q1 - q0);
    let rt1 = f * (3.0 * (q3 - q0) + 9.0 * (q1 - q2));
    let rt2 = 6.0 * (q0 + q2) - 12.0 * q1;

    let (mut p, mut d1, d2) = (rt0, f * (rt1 + rt2), 2.0 * f * rt1);
    (0..=it)
        .map(|_| {
            let out = p;
            p += d1;
            d1 += d2;
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const Q: [f32; 4] = [1.0, 4.0, -2.0, 3.0];

    fn close(got: &[f32], f: impl Fn(f32) -> f32) {
        let n = got.len() - 1;
        for (i, g) in got.iter().enumerate() {
            let want = f(i as f32 / n as f32);
            assert!((g - want).abs() < 1e-4, "{i}/{n}: {g} != {want}");
        }
    }

    #[test]
    fn positions_match_the_cubic() {
        let [a, b, c, d] = Q;
        for n in [1, 2, 12] {
            let got = forward_diff_bezier(Q, n);
            assert_eq!(got.len(), n + 1);
            close(&got, |t| {
                let s = 1.0 - t;
                s * s * s * a + 3.0 * s * s * t * b + 3.0 * s * t * t * c + t * t * t * d
            });
        }
    }

    #[test]
    fn tangents_match_the_derivative() {
        let [a, b, c, d] = Q;
        for n in [1, 2, 12] {
            close(&forward_diff_tangent_bezier(Q, n), |t| {
                let s = 1.0 - t;
                3.0 * (s * s * (b - a) + 2.0 * s * t * (c - b) + t * t * (d - c))
            });
        }
    }
}
