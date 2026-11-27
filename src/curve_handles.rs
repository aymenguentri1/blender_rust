//! Bezier handle calculation: `calchandleNurb_intern()` (curve.cc) for the auto, vector, align and free types.

pub const HD_FREE: u8 = 0;
pub const HD_AUTO: u8 = 1;
pub const HD_VECT: u8 = 2;
pub const HD_ALIGN: u8 = 3;
pub const HD_AUTO_ANIM: u8 = 4;
pub const HD_ALIGN_DOUBLESIDE: u8 = 5;

pub const HD_AUTOTYPE_NORMAL: u8 = 0;
pub const HD_AUTOTYPE_LOCKED_FINAL: u8 = 1;

/// `BEZT_FLAG_SELECT` (`SELECT`): the bit of `f1` that marks the first handle as selected.
pub const BEZT_FLAG_SELECT: u8 = 1;

pub const FCURVE_SMOOTH_NONE: u8 = 0;
pub const FCURVE_SMOOTH_CONT_ACCEL: u8 = 1;

/// The fields of `BezTriple` that the handle calculation reads or writes. `vec` is handle 1, the key, handle 2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BezTriple {
    pub vec: [[f32; 3]; 3],
    pub h1: u8,
    pub h2: u8,
    pub f1: u8,
    pub auto_handle_type: u8,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn len3(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}
/// `madd_v3_v3v3fl(r, a, b, f)`: `r = a + b * f`.
fn madd(a: [f32; 3], b: [f32; 3], f: f32) -> [f32; 3] {
    [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
}

fn is_auto(h: u8) -> bool {
    h == HD_AUTO || h == HD_AUTO_ANIM
}
fn is_align(h: u8) -> bool {
    h == HD_ALIGN || h == HD_ALIGN_DOUBLESIDE
}

/// `calchandleNurb_intern()`: recomputes the handles of `bezt` from its neighbours `prev` and `next`.
pub fn calc_handle(
    bezt: &mut BezTriple,
    prev: Option<&BezTriple>,
    next: Option<&BezTriple>,
    handle_sel_flag: u8,
    is_fcurve: bool,
    skip_align: bool,
    fcurve_smoothing: u8,
) {
    const EPS: f32 = 1e-5;

    /* assume normal handle until we check */
    bezt.auto_handle_type = HD_AUTOTYPE_NORMAL;

    if bezt.h1 == 0 && bezt.h2 == 0 {
        return;
    }
    if prev.is_none() && next.is_none() {
        /* Single point, no need to calculate handles. */
        return;
    }

    let p2 = bezt.vec[1];
    let p1 = match prev {
        Some(p) => p.vec[1],
        None => {
            let p3 = next.unwrap().vec[1];
            [2.0 * p2[0] - p3[0], 2.0 * p2[1] - p3[1], 2.0 * p2[2] - p3[2]]
        }
    };
    let p3 = match next {
        Some(n) => n.vec[1],
        None => [2.0 * p2[0] - p1[0], 2.0 * p2[1] - p1[1], 2.0 * p2[2] - p1[2]],
    };

    let dvec_a = sub(p2, p1);
    let dvec_b = sub(p3, p2);

    let (mut len_a, mut len_b) = if is_fcurve { (dvec_a[0], dvec_b[0]) } else { (len3(dvec_a), len3(dvec_b)) };
    if len_a == 0.0 {
        len_a = 1.0;
    }
    if len_b == 0.0 {
        len_b = 1.0;
    }

    if is_auto(bezt.h1) || is_auto(bezt.h2) {
        let tvec = [
            dvec_b[0] / len_b + dvec_a[0] / len_a,
            dvec_b[1] / len_b + dvec_a[1] / len_a,
            dvec_b[2] / len_b + dvec_a[2] / len_a,
        ];
        let mut len = if is_fcurve {
            if fcurve_smoothing != FCURVE_SMOOTH_NONE {
                /* force the horizontal handle size to be 1/3 of the key interval so that
                 * the X component of the parametric bezier curve is a linear spline */
                6.0 / 2.5614
            } else {
                tvec[0]
            }
        } else {
            len3(tvec)
        };
        len *= 2.5614;

        if len != 0.0 {
            /* Only for F-Curves. */
            let (mut leftviolate, mut rightviolate) = (false, false);

            if !is_fcurve || fcurve_smoothing == FCURVE_SMOOTH_NONE {
                len_a = len_a.min(5.0 * len_b);
                len_b = len_b.min(5.0 * len_a);
            }

            if is_auto(bezt.h1) {
                len_a /= len;
                bezt.vec[0] = madd(p2, tvec, -len_a);

                if bezt.h1 == HD_AUTO_ANIM && next.is_some() && prev.is_some() {
                    /* keep horizontal if extrema */
                    let (prev, next) = (prev.unwrap(), next.unwrap());
                    let ydiff1 = prev.vec[1][1] - bezt.vec[1][1];
                    let ydiff2 = next.vec[1][1] - bezt.vec[1][1];
                    if (ydiff1 <= 0.0 && ydiff2 <= 0.0) || (ydiff1 >= 0.0 && ydiff2 >= 0.0) {
                        bezt.vec[0][1] = bezt.vec[1][1];
                        bezt.auto_handle_type = HD_AUTOTYPE_LOCKED_FINAL;
                    } else if ydiff1 <= 0.0 {
                        /* handles should not be beyond y coord of two others */
                        if prev.vec[1][1] > bezt.vec[0][1] {
                            bezt.vec[0][1] = prev.vec[1][1];
                            leftviolate = true;
                        }
                    } else if prev.vec[1][1] < bezt.vec[0][1] {
                        bezt.vec[0][1] = prev.vec[1][1];
                        leftviolate = true;
                    }
                }
            }
            if is_auto(bezt.h2) {
                len_b /= len;
                bezt.vec[2] = madd(p2, tvec, len_b);

                if bezt.h2 == HD_AUTO_ANIM && next.is_some() && prev.is_some() {
                    let (prev, next) = (prev.unwrap(), next.unwrap());
                    let ydiff1 = prev.vec[1][1] - bezt.vec[1][1];
                    let ydiff2 = next.vec[1][1] - bezt.vec[1][1];
                    if (ydiff1 <= 0.0 && ydiff2 <= 0.0) || (ydiff1 >= 0.0 && ydiff2 >= 0.0) {
                        bezt.vec[2][1] = bezt.vec[1][1];
                        bezt.auto_handle_type = HD_AUTOTYPE_LOCKED_FINAL;
                    } else if ydiff1 <= 0.0 {
                        if next.vec[1][1] < bezt.vec[2][1] {
                            bezt.vec[2][1] = next.vec[1][1];
                            rightviolate = true;
                        }
                    } else if next.vec[1][1] > bezt.vec[2][1] {
                        bezt.vec[2][1] = next.vec[1][1];
                        rightviolate = true;
                    }
                }
            }
            if leftviolate || rightviolate {
                /* align left handle: simple 2d calculation */
                let h1_x = bezt.vec[0][0] - p2[0];
                let h2_x = p2[0] - bezt.vec[2][0];
                if leftviolate {
                    bezt.vec[2][1] = p2[1] + ((p2[1] - bezt.vec[0][1]) / h1_x) * h2_x;
                } else {
                    bezt.vec[0][1] = p2[1] + ((p2[1] - bezt.vec[2][1]) / h2_x) * h1_x;
                }
            }
        }
    }

    if bezt.h1 == HD_VECT {
        bezt.vec[0] = madd(p2, dvec_a, -1.0 / 3.0);
    }
    if bezt.h2 == HD_VECT {
        bezt.vec[2] = madd(p2, dvec_b, 1.0 / 3.0);
    }

    if skip_align
        /* When one handle is free, aligning makes no sense, see: #35952 */
        || bezt.h1 == HD_FREE || bezt.h2 == HD_FREE
        /* Also when no handles are aligned, skip this step. */
        || !(is_align(bezt.h1) || is_align(bezt.h2))
    {
        return;
    }

    let h1 = bezt.vec[0];
    let h2 = bezt.vec[2];
    let mut len_a = len3(sub(p2, h1));
    let mut len_b = len3(sub(p2, h2));
    if len_a == 0.0 {
        len_a = 1.0;
    }
    if len_b == 0.0 {
        len_b = 1.0;
    }
    let len_ratio = len_a / len_b;

    let align_h2 = |b: &mut BezTriple| {
        if len_a > EPS {
            let len = 1.0 / len_ratio;
            for k in 0..3 {
                b.vec[2][k] = p2[k] + len * (p2[k] - b.vec[0][k]);
            }
        }
    };
    let align_h1 = |b: &mut BezTriple| {
        if len_b > EPS {
            let len = len_ratio;
            for k in 0..3 {
                b.vec[0][k] = p2[k] + len * (p2[k] - b.vec[2][k]);
            }
        }
    };
    if bezt.f1 & handle_sel_flag != 0 {
        /* order of calculation */
        if is_align(bezt.h2) {
            align_h2(bezt);
        }
        if is_align(bezt.h1) {
            align_h1(bezt);
        }
    } else {
        if is_align(bezt.h1) {
            align_h1(bezt);
        }
        if is_align(bezt.h2) {
            align_h2(bezt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key `(x, y, z)` with handles at `(x - 1, y + .5, z + .25)` and `(x + 2, y - .5, z - .75)`.
    fn mk(x: f32, y: f32, z: f32, h1: u8, h2: u8) -> BezTriple {
        BezTriple {
            vec: [[x - 1.0, y + 0.5, z + 0.25], [x, y, z], [x + 2.0, y - 0.5, z - 0.75]],
            h1,
            h2,
            f1: 0,
            auto_handle_type: HD_AUTOTYPE_NORMAL,
        }
    }

    fn close(got: &BezTriple, want: [[f32; 3]; 3], auto_type: u8) {
        for i in 0..3 {
            for k in 0..3 {
                assert!((got.vec[i][k] - want[i][k]).abs() < 1e-5, "vec[{i}][{k}]: {:?} vs {want:?}", got.vec);
            }
        }
        assert_eq!(got.auto_handle_type, auto_type);
    }

    /* Expected values are printed by the C `calchandleNurb_intern()` compiled on its own with the same inputs. */
    fn pmn() -> (BezTriple, BezTriple, BezTriple) {
        (mk(0.0, 0.0, 0.0, HD_AUTO, HD_AUTO), mk(2.0, 3.0, 1.0, HD_AUTO, HD_AUTO), mk(5.0, 1.0, -2.0, HD_AUTO, HD_AUTO))
    }

    #[test]
    fn auto_handles_of_a_middle_first_and_last_key() {
        let (p, m, n) = pmn();
        let mut b = m;
        calc_handle(&mut b, Some(&p), Some(&n), 1, false, false, 0);
        close(&b, [[0.668034077, 2.5741539, 1.42239583], [2.0, 3.0, 1.0], [3.66970778, 3.53382635, 0.470498741]], 0);
        let mut b = p;
        calc_handle(&mut b, None, Some(&m), 1, false, false, 0);
        close(&b, [[-0.780823052, -1.17123461, -0.390411526], [0.0; 3], [0.780823052, 1.17123461, 0.390411526]], 0);
        let mut b = n;
        calc_handle(&mut b, Some(&m), None, 1, false, false, 0);
        close(&b, [[3.82876539, 1.78082311, -0.828765392], [5.0, 1.0, -2.0], [6.17123461, 0.219176888, -3.17123461]], 0);
    }

    #[test]
    fn vector_handles_point_a_third_of_the_way_to_the_neighbours() {
        let (p, mut b, n) = pmn();
        b.h1 = HD_VECT;
        b.h2 = HD_VECT;
        calc_handle(&mut b, Some(&p), Some(&n), 1, false, false, 0);
        close(&b, [[1.33333325, 2.0, 0.666666627], [2.0, 3.0, 1.0], [3.0, 2.33333325, 0.0]], 0);
    }

    #[test]
    fn free_handles_are_left_alone() {
        let (p, mut b, n) = pmn();
        b.h1 = HD_FREE;
        b.h2 = HD_FREE;
        calc_handle(&mut b, Some(&p), Some(&n), 1, false, false, 0);
        close(&b, [[1.0, 3.5, 1.25], [2.0, 3.0, 1.0], [4.0, 2.5, 0.25]], 0);
        /* One free handle also stops the aligned one from following. */
        let mut b = mk(2.0, 3.0, 1.0, HD_FREE, HD_ALIGN);
        calc_handle(&mut b, Some(&p), Some(&n), 1, false, false, 0);
        close(&b, [[1.0, 3.5, 1.25], [2.0, 3.0, 1.0], [4.0, 2.5, 0.25]], 0);
    }

    #[test]
    fn aligned_handles_follow_the_selected_one() {
        let (p, _, n) = pmn();
        let mut b = mk(2.0, 3.0, 1.0, HD_ALIGN, HD_ALIGN);
        b.f1 = BEZT_FLAG_SELECT;
        calc_handle(&mut b, Some(&p), Some(&n), BEZT_FLAG_SELECT, false, false, 0);
        close(&b, [[1.0, 3.5, 1.25], [2.0, 3.0, 1.0], [3.91485405, 2.04257298, 0.521286488]], 0);
        let mut b = mk(2.0, 3.0, 1.0, HD_ALIGN, HD_ALIGN);
        calc_handle(&mut b, Some(&p), Some(&n), BEZT_FLAG_SELECT, false, false, 0);
        close(&b, [[0.955533981, 3.2611165, 1.39167476], [2.0, 3.0, 1.0], [4.0, 2.5, 0.25]], 0);
        /* An auto handle next to an aligned one: the auto one is computed first, the aligned one follows it. */
        let mut b = mk(2.0, 3.0, 1.0, HD_AUTO, HD_ALIGN);
        b.f1 = BEZT_FLAG_SELECT;
        calc_handle(&mut b, Some(&p), Some(&n), BEZT_FLAG_SELECT, false, false, 0);
        close(&b, [[0.668034077, 2.5741539, 1.42239583], [2.0, 3.0, 1.0], [4.00028467, 3.63951588, 0.365665555]], 0);
        /* `skip_align` keeps the auto result as it is. */
        let (p, m, n) = pmn();
        let mut b = m;
        calc_handle(&mut b, Some(&p), Some(&n), 1, false, true, 0);
        close(&b, [[0.668034077, 2.5741539, 1.42239583], [2.0, 3.0, 1.0], [3.66970778, 3.53382635, 0.470498741]], 0);
    }

    #[test]
    fn auto_anim_fcurve_handles_are_clamped() {
        let fp = mk(0.0, 0.0, 0.0, HD_AUTO_ANIM, HD_AUTO_ANIM);
        let fn_ = mk(5.0, 1.0, 0.0, HD_AUTO_ANIM, HD_AUTO_ANIM);
        let key = |y: f32| mk(2.0, y, 0.0, HD_AUTO_ANIM, HD_AUTO_ANIM);
        /* A key above both neighbours is an extremum: flat handles, locked. */
        let mut b = key(5.0);
        calc_handle(&mut b, Some(&fp), Some(&fn_), 1, true, false, FCURVE_SMOOTH_NONE);
        close(&b, [[1.21917701, 5.0, 0.0], [2.0, 5.0, 0.0], [3.17123461, 5.0, 0.0]], HD_AUTOTYPE_LOCKED_FINAL);
        let mut b = key(3.0);
        calc_handle(&mut b, Some(&fp), Some(&fn_), 1, true, false, FCURVE_SMOOTH_NONE);
        close(&b, [[1.21917701, 3.0, 0.0], [2.0, 3.0, 0.0], [3.17123461, 3.0, 0.0]], HD_AUTOTYPE_LOCKED_FINAL);
        /* Between its neighbours the handles are clamped to their y range and re-aligned. */
        let mut b = key(0.5);
        calc_handle(&mut b, Some(&fp), Some(&fn_), 1, true, false, FCURVE_SMOOTH_NONE);
        close(&b, [[1.21917701, 0.337328553, 0.0], [2.0, 0.5, 0.0], [3.17123461, 0.74400723, 0.0]], HD_AUTOTYPE_NORMAL);
        /* Continuous acceleration smoothing makes the x handle length a third of the interval. */
        let mut b = key(3.0);
        calc_handle(&mut b, Some(&fp), Some(&fn_), 1, true, false, FCURVE_SMOOTH_CONT_ACCEL);
        close(&b, [[1.33333325, 3.0, 0.0], [2.0, 3.0, 0.0], [3.0, 3.0, 0.0]], HD_AUTOTYPE_LOCKED_FINAL);
    }

    #[test]
    fn a_single_key_or_two_free_handles_do_nothing() {
        let mut b = mk(1.0, 2.0, 3.0, HD_AUTO, HD_AUTO);
        let before = b;
        calc_handle(&mut b, None, None, 1, false, false, 0);
        assert_eq!(b, before);
        let (p, _, n) = pmn();
        let mut b = mk(1.0, 2.0, 3.0, 0, 0);
        let before = b;
        calc_handle(&mut b, Some(&p), Some(&n), 1, false, false, 0);
        assert_eq!(b, before);
    }
}
