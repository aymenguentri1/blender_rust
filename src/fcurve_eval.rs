//! F-curve evaluation (constant / linear / Bezier), ported from
//! source/blender/blenkernel/intern/fcurve.cc (fcurve_eval_keyframes*, findzero, berekeny).

/// Keyframe interpolation (subset of `eBezTriple_Interpolation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ipo {
    Const,
    Lin,
    Bez,
}

/// Extrapolation (`FCURVE_EXTRAPOLATE_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extend {
    Constant,
    Linear,
}

/// Bezier triple: vec[0]=left handle, vec[1]=key, vec[2]=right handle, each (x, y).
#[derive(Clone, Copy, Debug)]
pub struct BezTriple {
    pub vec: [[f32; 2]; 3],
    pub ipo: Ipo,
}

const SMALL: f32 = -1.0e-10;

fn sqrt3d(d: f64) -> f64 {
    if d == 0.0 {
        0.0
    } else if d < 0.0 {
        -((-d).ln() / 3.0).exp()
    } else {
        (d.ln() / 3.0).exp()
    }
}

fn in_range(v: f32) -> bool {
    v >= SMALL && v <= 1.000001
}

/// Roots in [0,1] of c0 + c1 x + c2 x^2 + c3 x^3 (`solve_cubic`). Returns count, roots in `o`.
fn solve_cubic(c0: f64, c1: f64, c2: f64, c3: f64, o: &mut [f32; 3]) -> usize {
    let mut nr = 0usize;
    if c3 != 0.0 {
        let a = c2 / c3 / 3.0;
        let b = c1 / c3;
        let c = c0 / c3;
        let p = b / 3.0 - a * a;
        let q = (2.0 * a * a * a - a * b + c) / 2.0;
        let d = q * q + p * p * p;
        if d > 0.0 {
            let t = d.sqrt();
            o[0] = (sqrt3d(-q + t) + sqrt3d(-q - t) - a) as f32;
            return usize::from(in_range(o[0]));
        }
        if d == 0.0 {
            let t = sqrt3d(-q);
            o[0] = (2.0 * t - a) as f32;
            if in_range(o[0]) {
                nr += 1;
            }
            o[nr] = (-t - a) as f32;
            return if in_range(o[nr]) { nr + 1 } else { nr };
        }
        let phi = (-q / (-(p * p * p)).sqrt()).acos();
        let t = (-p).sqrt();
        let p = (phi / 3.0).cos();
        let q = (3.0 - 3.0 * p * p).sqrt();
        o[0] = (2.0 * t * p - a) as f32;
        if in_range(o[0]) {
            nr += 1;
        }
        o[nr] = (-t * (p + q) - a) as f32;
        if in_range(o[nr]) {
            nr += 1;
        }
        o[nr] = (-t * (p - q) - a) as f32;
        return if in_range(o[nr]) { nr + 1 } else { nr };
    }
    let (a, b, c) = (c2, c1, c0);
    if a != 0.0 {
        let p = b * b - 4.0 * a * c;
        if p > 0.0 {
            let p = p.sqrt();
            o[0] = ((-b - p) / (2.0 * a)) as f32;
            if in_range(o[0]) {
                nr += 1;
            }
            o[nr] = ((-b + p) / (2.0 * a)) as f32;
            return if in_range(o[nr]) { nr + 1 } else { nr };
        }
        if p == 0.0 {
            o[0] = (-b / (2.0 * a)) as f32;
            if in_range(o[0]) {
                return 1;
            }
        }
        return 0;
    }
    if b != 0.0 {
        o[0] = (-c / b) as f32;
        return usize::from(in_range(o[0]));
    }
    if c == 0.0 {
        o[0] = 0.0;
        return 1;
    }
    0
}

fn findzero(x: f32, q0: f32, q1: f32, q2: f32, q3: f32, o: &mut [f32; 3]) -> usize {
    let c0 = f64::from(q0 - x);
    let c1 = f64::from(3.0 * (q1 - q0));
    let c2 = f64::from(3.0 * (q0 - 2.0 * q1 + q2));
    let c3 = f64::from(q3 - q0 + 3.0 * (q1 - q2));
    solve_cubic(c0, c1, c2, c3, o)
}

fn berekeny(f1: f32, f2: f32, f3: f32, f4: f32, t: f32) -> f32 {
    let c0 = f1;
    let c1 = 3.0 * (f2 - f1);
    let c2 = 3.0 * (f1 - 2.0 * f2 + f3);
    let c3 = f4 - f1 + 3.0 * (f2 - f3);
    c0 + t * c1 + t * t * c2 + t * t * t * c3
}

/// `BKE_fcurve_correct_bezpart`: stop handles overshooting the neighbouring key times.
pub fn correct_bezpart(v1: [f32; 2], v2: &mut [f32; 2], v3: &mut [f32; 2], v4: [f32; 2]) {
    let h1 = [v1[0] - v2[0], v1[1] - v2[1]];
    let h2 = [v4[0] - v3[0], v4[1] - v3[1]];
    let len = v4[0] - v1[0];
    let len1 = h1[0].abs();
    let len2 = h2[0].abs();
    if len1 + len2 == 0.0 {
        return;
    }
    if len1 > len {
        let fac = len / len1;
        *v2 = [v1[0] - fac * h1[0], v1[1] - fac * h1[1]];
    }
    if len2 > len {
        let fac = len / len2;
        *v3 = [v4[0] - fac * h2[0], v4[1] - fac * h2[1]];
    }
}

fn is_eqt(a: f32, b: f32, c: f32) -> bool {
    (a - b).abs() <= c
}

/// `BKE_fcurve_bezt_binarysearch_index_ex`: (index, exact).
pub fn binarysearch_index(bezts: &[BezTriple], frame: f32, threshold: f32) -> (usize, bool) {
    let n = bezts.len();
    if n == 0 {
        return (0, false);
    }
    let first = bezts[0].vec[1][0];
    if is_eqt(frame, first, threshold) {
        return (0, true);
    }
    if frame < first {
        return (0, false);
    }
    let last = bezts[n - 1].vec[1][0];
    if is_eqt(frame, last, threshold) {
        return (n - 1, true);
    }
    if frame > last {
        return (n, false);
    }
    let (mut start, mut end) = (0i64, n as i64);
    while start <= end {
        let mid = start + (end - start) / 2;
        let midfra = bezts[(mid as usize).min(n - 1)].vec[1][0];
        if is_eqt(frame, midfra, threshold) {
            return (mid as usize, true);
        }
        if frame > midfra {
            start = mid + 1;
        } else {
            end = mid - 1;
        }
    }
    (start as usize, false)
}

fn extrapolate(bezts: &[BezTriple], extend: Extend, evaltime: f32, last: bool) -> f32 {
    let n = bezts.len();
    let (ei, dir): (usize, i64) = if last { (n - 1, -1) } else { (0, 1) };
    let e = &bezts[ei];
    if e.ipo == Ipo::Const || extend == Extend::Constant {
        return e.vec[1][1];
    }
    if e.ipo == Ipo::Lin {
        if n == 1 {
            return e.vec[1][1];
        }
        let nb = &bezts[(ei as i64 + dir) as usize];
        let dx = e.vec[1][0] - evaltime;
        let fac = nb.vec[1][0] - e.vec[1][0];
        if fac == 0.0 {
            return e.vec[1][1];
        }
        let fac = (nb.vec[1][1] - e.vec[1][1]) / fac;
        return e.vec[1][1] - fac * dx;
    }
    let h = if dir > 0 { 0 } else { 2 };
    let dx = e.vec[1][0] - evaltime;
    let fac = e.vec[1][0] - e.vec[h][0];
    if fac == 0.0 {
        return e.vec[1][1];
    }
    let fac = (e.vec[1][1] - e.vec[h][1]) / fac;
    e.vec[1][1] - fac * dx
}

fn interpolate(bezts: &[BezTriple], evaltime: f32) -> f32 {
    let eps = 1.0e-8_f32;
    let (a, exact) = binarysearch_index(bezts, evaltime, 0.0001);
    let bezt = &bezts[a.min(bezts.len() - 1)];
    if exact {
        return bezt.vec[1][1];
    }
    let prev = if a > 0 { &bezts[a - 1] } else { bezt };
    if (bezt.vec[1][0] - evaltime).abs() < eps {
        return bezt.vec[1][1];
    }
    if evaltime < prev.vec[1][0] || bezt.vec[1][0] < evaltime {
        return 0.0;
    }
    let begin = prev.vec[1][1];
    let change = bezt.vec[1][1] - prev.vec[1][1];
    let duration = bezt.vec[1][0] - prev.vec[1][0];
    let time = evaltime - prev.vec[1][0];
    if prev.ipo == Ipo::Const || duration == 0.0 {
        return prev.vec[1][1];
    }
    match prev.ipo {
        Ipo::Bez => {
            let v1 = prev.vec[1];
            let mut v2 = prev.vec[2];
            let mut v3 = bezt.vec[0];
            let v4 = bezt.vec[1];
            if (v1[1] - v4[1]).abs() < f32::EPSILON
                && (v2[1] - v3[1]).abs() < f32::EPSILON
                && (v3[1] - v4[1]).abs() < f32::EPSILON
            {
                return v1[1];
            }
            correct_bezpart(v1, &mut v2, &mut v3, v4);
            let mut opl = [0.0f32; 3];
            if findzero(evaltime, v1[0], v2[0], v3[0], v4[0], &mut opl) == 0 {
                return 0.0;
            }
            berekeny(v1[1], v2[1], v3[1], v4[1], opl[0])
        }
        // BLI_easing_linear_ease: change * time / duration + begin
        _ => change * time / duration + begin,
    }
}

/// `evaluate_fcurve_unmodified` for keyframed curves (no modifiers, no drivers).
pub fn evaluate_keyframes(bezts: &[BezTriple], extend: Extend, evaltime: f32) -> f32 {
    if bezts.is_empty() {
        return 0.0;
    }
    if evaltime <= bezts[0].vec[1][0] {
        return extrapolate(bezts, extend, evaltime, false);
    }
    if bezts[bezts.len() - 1].vec[1][0] <= evaltime {
        return extrapolate(bezts, extend, evaltime, true);
    }
    interpolate(bezts, evaltime)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(x: f32, y: f32, ipo: Ipo) -> BezTriple {
        BezTriple { vec: [[x - 1.0, y], [x, y], [x + 1.0, y]], ipo }
    }

    #[test]
    fn constant_holds_previous_key() {
        let k = [key(0.0, 1.0, Ipo::Const), key(10.0, 5.0, Ipo::Const)];
        assert_eq!(evaluate_keyframes(&k, Extend::Constant, 4.0), 1.0);
        assert_eq!(evaluate_keyframes(&k, Extend::Constant, 10.0), 5.0);
    }

    #[test]
    fn linear_interpolates_and_extrapolates() {
        let k = [key(0.0, 0.0, Ipo::Lin), key(10.0, 10.0, Ipo::Lin)];
        assert!((evaluate_keyframes(&k, Extend::Constant, 2.5) - 2.5).abs() < 1e-6);
        assert_eq!(evaluate_keyframes(&k, Extend::Constant, 20.0), 10.0);
        assert!((evaluate_keyframes(&k, Extend::Linear, 20.0) - 20.0).abs() < 1e-5);
        assert!((evaluate_keyframes(&k, Extend::Linear, -5.0) + 5.0).abs() < 1e-5);
    }

    #[test]
    fn bezier_symmetric_midpoint_and_ends() {
        let mut a = key(0.0, 0.0, Ipo::Bez);
        let mut b = key(10.0, 10.0, Ipo::Bez);
        a.vec[2] = [3.0, 0.0];
        b.vec[0] = [7.0, 10.0];
        let k = [a, b];
        assert!((evaluate_keyframes(&k, Extend::Constant, 5.0) - 5.0).abs() < 1e-3);
        assert!(evaluate_keyframes(&k, Extend::Constant, 1.0) < 1.0);
        assert_eq!(evaluate_keyframes(&k, Extend::Constant, 0.0), 0.0);
    }

    #[test]
    fn bezier_flat_returns_shared_value() {
        let k = [key(0.0, 2.0, Ipo::Bez), key(5.0, 2.0, Ipo::Bez)];
        assert_eq!(evaluate_keyframes(&k, Extend::Constant, 2.0), 2.0);
    }

    #[test]
    fn binary_search_flags() {
        let k = [key(0.0, 0.0, Ipo::Lin), key(5.0, 0.0, Ipo::Lin), key(9.0, 0.0, Ipo::Lin)];
        assert_eq!(binarysearch_index(&k, 5.0, 0.0001), (1, true));
        assert_eq!(binarysearch_index(&k, 7.0, 0.0001), (2, false));
        assert_eq!(binarysearch_index(&k, 12.0, 0.0001), (3, false));
    }
}
