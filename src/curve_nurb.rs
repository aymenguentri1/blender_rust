//! NURBS curve evaluation: `basisNurb()` and `BKE_nurb_makeCurve()` (curve.cc).

use crate::curve_knots::{knot_count, CU_NURB_CYCLIC};

/// The parts of `BPoint` that `BKE_nurb_makeCurve` reads: `vec` is x, y, z and the weight w.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NurbPoint {
    pub vec: [f32; 4],
    /// `f1`: the selection flag.
    pub f1: u8,
    pub tilt: f32,
    pub radius: f32,
    pub weight: f32,
}

/// One evaluated point (the coord, tilt, radius and weight arrays of `BKE_nurb_makeCurve`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CurveSample {
    pub co: [f32; 3],
    pub tilt: f32,
    pub radius: f32,
    pub weight: f32,
}

/// `basisNurb()`: fills `basis` (at least `order + pnts` long) and returns the `(start, end)` range of
/// non-zero basis functions.
pub fn basis_nurb(t: f32, order: usize, pnts: usize, knots: &[f32], basis: &mut [f32]) -> (usize, usize) {
    let orderpluspnts = order + pnts;
    let opp2 = orderpluspnts - 1;
    let (mut i1, mut i2) = (0usize, 0usize);

    /* this is for float inaccuracy */
    let t = t.clamp(knots[0], knots[opp2]);

    /* this part is order '1' */
    let o2 = order + 1;
    let mut i = 0;
    while i < opp2 {
        if knots[i] != knots[i + 1] && t >= knots[i] && t <= knots[i + 1] {
            basis[i] = 1.0;
            i1 = i.saturating_sub(o2);
            i2 = i;
            i += 1;
            while i < opp2 {
                basis[i] = 0.0;
                i += 1;
            }
            break;
        }
        basis[i] = 0.0;
        i += 1;
    }
    basis[i] = 0.0;

    /* this is order 2, 3, ... */
    for j in 2..=order {
        if i2 + j >= orderpluspnts {
            i2 = opp2 - j;
        }
        for i in i1..=i2 {
            let d = if basis[i] != 0.0 {
                ((t - knots[i]) * basis[i]) / (knots[i + j - 1] - knots[i])
            } else {
                0.0
            };
            let e = if basis[i + 1] != 0.0 {
                ((knots[i + j] - t) * basis[i + 1]) / (knots[i + j] - knots[i + 1])
            } else {
                0.0
            };
            basis[i] = d + e;
        }
    }

    let (mut start, mut end) = (1000usize, 0usize);
    for i in i1..=i2 {
        if basis[i] > 0.0 {
            end = i;
            if start == 1000 {
                start = i;
            }
        }
    }
    (start, end)
}

/// `BKE_nurb_makeCurve()`: `resolu` samples per segment. Returns nothing for an invalid curve
/// (no knots, `order > pntsu`, no points or no samples), like the C early outs.
pub fn make_curve(points: &[NurbPoint], knots: &[f32], order: usize, flag: i16, resolu: usize) -> Vec<CurveSample> {
    const EPS: f32 = 1e-6;
    let pntsu = points.len();
    let is_cyclic = flag & CU_NURB_CYCLIC != 0;
    if knots.is_empty() || order > pntsu || pntsu == 0 {
        return Vec::new();
    }
    /* SEGMENTSU() */
    let resolu = resolu * (pntsu - usize::from(!is_cyclic));
    if resolu == 0 {
        return Vec::new();
    }

    let ustart = knots[order - 1];
    let uend = if is_cyclic { knots[pntsu + order - 1] } else { knots[pntsu] };
    let ustep = (uend - ustart) / (resolu - usize::from(!is_cyclic)) as f32;
    let cycl = if is_cyclic { order - 1 } else { 0 };

    let mut basisu = vec![0.0f32; knot_count(pntsu, order, flag)];
    let mut out = Vec::with_capacity(resolu);
    let mut u = ustart;
    for _ in 0..resolu {
        let (istart, iend) = basis_nurb(u, order, pntsu + cycl, knots, &mut basisu);
        let point = |i: usize| &points[if i >= pntsu { i - pntsu } else { i }];

        /* calc sum */
        let mut sum: Vec<f32> = (istart..=iend).map(|i| basisu[i] * point(i).vec[3]).collect();
        let sumdiv: f32 = sum.iter().sum();
        if sumdiv != 0.0 && (sumdiv < 1.0 - EPS || sumdiv > 1.0 + EPS) {
            /* is normalizing needed? */
            for s in &mut sum {
                *s /= sumdiv;
            }
        }

        let mut sample = CurveSample::default();
        for (k, i) in (istart..=iend).enumerate() {
            let f = sum[k];
            if f != 0.0 {
                let bp = point(i);
                for a in 0..3 {
                    sample.co[a] += bp.vec[a] * f;
                }
                sample.tilt += f * bp.tilt;
                sample.radius += f * bp.radius;
                sample.weight += f * bp.weight;
            }
        }
        out.push(sample);
        u += ustep;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve_knots::{calc_knots, CU_NURB_ENDPOINT};

    fn pt(x: f32, y: f32, z: f32, w: f32) -> NurbPoint {
        NurbPoint { vec: [x, y, z, w], f1: 0, tilt: 0.0, radius: 1.0, weight: 1.0 }
    }

    #[test]
    fn endpoint_nurbs_starts_and_ends_on_its_control_points() {
        let pts = [pt(0.0, 0.0, 0.0, 1.0), pt(1.0, 2.0, 0.0, 1.0), pt(3.0, 3.0, 1.0, 1.0), pt(4.0, 1.0, 2.0, 1.0), pt(5.0, 0.0, 3.0, 1.0)];
        let knots = calc_knots(5, 4, CU_NURB_ENDPOINT);
        let c = make_curve(&pts, &knots, 4, CU_NURB_ENDPOINT, 8);
        assert_eq!(c.len(), 32);
        for (got, want) in [(c[0].co, pts[0].vec), (c[31].co, pts[4].vec)] {
            for a in 0..3 {
                assert!((got[a] - want[a]).abs() < 1e-5, "{got:?} vs {want:?}");
            }
        }
        assert!((c[0].radius - 1.0).abs() < 1e-5);
    }

    #[test]
    fn weighted_circle_points_have_the_expected_radius() {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let pts = [
            pt(1.0, 0.0, 0.0, 1.0),
            pt(1.0, 1.0, 0.0, s),
            pt(0.0, 1.0, 0.0, 1.0),
            pt(-1.0, 1.0, 0.0, s),
            pt(-1.0, 0.0, 0.0, 1.0),
            pt(-1.0, -1.0, 0.0, s),
            pt(0.0, -1.0, 0.0, 1.0),
            pt(1.0, -1.0, 0.0, s),
            pt(1.0, 0.0, 0.0, 1.0),
        ];
        let knots = [0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0];
        let c = make_curve(&pts, &knots, 3, 0, 6);
        assert_eq!(c.len(), 48);
        for p in &c {
            let r = (p.co[0] * p.co[0] + p.co[1] * p.co[1]).sqrt();
            assert!((r - 1.0).abs() < 1e-4, "radius {r} at {:?}", p.co);
        }
    }

    #[test]
    fn basis_is_a_partition_of_unity() {
        let knots = calc_knots(5, 4, CU_NURB_ENDPOINT);
        let mut basis = vec![0.0; knots.len()];
        let (s, e) = basis_nurb(1.3, 4, 5, &knots, &mut basis);
        let sum: f32 = basis[s..=e].iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "{sum}");
    }

    #[test]
    fn invalid_curves_give_no_samples() {
        let pts = [pt(0.0, 0.0, 0.0, 1.0), pt(1.0, 0.0, 0.0, 1.0)];
        assert!(make_curve(&pts, &[], 2, 0, 4).is_empty());
        assert!(make_curve(&pts, &[0.0; 6], 3, 0, 4).is_empty());
        assert!(make_curve(&pts, &calc_knots(2, 2, 0), 2, 0, 0).is_empty());
    }
}
