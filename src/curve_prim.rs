//! Curve primitives: `ED_curve_add_nurbs_primitive()` (editcurve_add.cc) for the Bezier curve and circle,
//! the NURBS curve and circle, and the 5 point path, plus `BKE_nurb_handles_calc()` (curve.cc).
//!
//! Deviations: only the view independent case (`xzproj == 0`, no `rv3d`) is ported, and the surface, tube,
//! sphere and donut primitives (which call back into this function) are not.

use crate::curve_handles::{calc_handle, BezTriple, HD_ALIGN, HD_AUTO, BEZT_FLAG_SELECT};
use crate::curve_knots::{calc_knots, CU_NURB_BEZIER, CU_NURB_CYCLIC, CU_NURB_ENDPOINT};
use crate::curve_nurb::NurbPoint;

/// `eNurbType` and `CU_TYPE`.
pub const CU_POLY: i32 = 0;
pub const CU_BEZIER: i32 = 1;
pub const CU_NURBS: i32 = 4;
pub const CU_TYPE: i32 = CU_POLY | CU_BEZIER | CU_NURBS;
/// `CU_PRIM_*`, the primitive kind in `type & CU_PRIMITIVE`.
pub const CU_PRIM_CURVE: i32 = 0x100;
pub const CU_PRIM_CIRCLE: i32 = 0x200;
pub const CU_PRIM_PATH: i32 = 0x700;
pub const CU_PRIMITIVE: i32 = 0xF00;

/// `SELECT` / `BEZT_FLAG_SELECT`.
const SELECT: u8 = BEZT_FLAG_SELECT;

/// `nurbcircle`: the control points of the NURBS circle (x, y).
const NURBCIRCLE: [[f32; 2]; 8] =
    [[0.0, -1.0], [-1.0, -1.0], [-1.0, 0.0], [-1.0, 1.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [1.0, -1.0]];

/// A `BezTriple` with the fields `ED_curve_add_nurbs_primitive()` sets besides the ones of `calc_handle`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BezPoint {
    pub bez: BezTriple,
    pub f2: u8,
    pub f3: u8,
    pub radius: f32,
    pub weight: f32,
}

/// The fields of `Nurb` the primitives fill in.
#[derive(Clone, Debug, PartialEq)]
pub struct Nurb {
    pub nurb_type: i32,
    pub pntsu: usize,
    pub pntsv: usize,
    pub orderu: usize,
    pub flagu: i16,
    pub resolu: i32,
    pub bezt: Vec<BezPoint>,
    pub bp: Vec<NurbPoint>,
    pub knotsu: Option<Vec<f32>>,
}

/// `mul_m4_v3()`: `mat` is column major like Blender's, the translation is `mat[3]`.
fn mul_m4_v3(m: &[[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] {
    let mut r = [0.0; 3];
    for (i, r) in r.iter_mut().enumerate() {
        *r = m[0][i] * v[0] + m[1][i] * v[1] + m[2][i] * v[2] + m[3][i];
    }
    r
}

/// `BKE_nurb_handles_calc()`: `calchandlesNurb_intern(nu, SELECT, false)`.
pub fn handles_calc(nu: &mut Nurb) {
    if nu.nurb_type != CU_BEZIER || nu.pntsu < 2 {
        return;
    }
    let cyclic = nu.flagu & CU_NURB_CYCLIC != 0;
    let n = nu.pntsu;
    let mut prev = if cyclic { Some(n - 1) } else { None };
    let mut next = Some(1);
    let mut a = n;
    let mut i = 0;
    while a > 0 {
        a -= 1;
        let p = prev.map(|k| nu.bezt[k].bez);
        let nx = next.map(|k| nu.bezt[k].bez);
        calc_handle(&mut nu.bezt[i].bez, p.as_ref(), nx.as_ref(), SELECT, false, false, 0);
        prev = Some(i);
        if a == 1 {
            next = if cyclic { Some(0) } else { None };
        } else if let Some(k) = next {
            next = Some(k + 1);
        }
        i += 1;
    }
}

fn bezt(h: u8, vec: [[f32; 3]; 3], radius: f32, weight: f32) -> BezPoint {
    BezPoint {
        bez: BezTriple { vec, h1: h, h2: h, f1: SELECT, auto_handle_type: 0 },
        f2: SELECT,
        f3: SELECT,
        radius,
        weight,
    }
}

/// A selected control point at `v` with weight 1 in the 4th coordinate.
fn bpoint(v: [f32; 3], w: f32) -> NurbPoint {
    NurbPoint { vec: [v[0], v[1], v[2], w], f1: SELECT, tilt: 0.0, radius: 1.0, weight: 1.0 }
}

fn transform_all(mat: &[[f32; 4]; 4], bp: &mut [NurbPoint]) {
    for p in bp {
        let v = mul_m4_v3(mat, [p.vec[0], p.vec[1], p.vec[2]]);
        p.vec[..3].copy_from_slice(&v);
    }
}

/// `ED_curve_add_nurbs_primitive()`: the new `Nurb` of `ptype` (`CU_BEZIER | CU_PRIM_CURVE`, ...) transformed by
/// `mat`, with the curve resolution `resolu`. `None` for the primitives that are not ported.
pub fn add_nurbs_primitive(mat: &[[f32; 4]; 4], ptype: i32, resolu: i32) -> Option<Nurb> {
    const GRID: f32 = 1.0;
    let cutype = ptype & CU_TYPE;
    let stype = ptype & CU_PRIMITIVE;
    let mut nu = Nurb {
        nurb_type: cutype,
        pntsu: 0,
        pntsv: 0,
        orderu: 0,
        flagu: 0,
        resolu,
        bezt: Vec::new(),
        bp: Vec::new(),
        knotsu: None,
    };
    let tr3 = |v: [[f32; 3]; 3]| [mul_m4_v3(mat, v[0]), mul_m4_v3(mat, v[1]), mul_m4_v3(mat, v[2])];

    match stype {
        CU_PRIM_CURVE => {
            if cutype == CU_BEZIER {
                nu.pntsu = 2;
                let first = [[-1.5 * GRID, -0.5 * GRID, 0.0], [-GRID, 0.0, 0.0], [-0.5 * GRID, 0.5 * GRID, 0.0]];
                let second = [[0.0, 0.0, 0.0], [GRID, 0.0, 0.0], [GRID * 2.0, 0.0, 0.0]];
                /* The first key keeps `weight` 0 (zeroed allocation), only the second sets it. */
                nu.bezt = vec![bezt(HD_ALIGN, tr3(first), 1.0, 0.0), bezt(HD_ALIGN, tr3(second), 1.0, 1.0)];
                handles_calc(&mut nu);
            } else {
                nu.pntsu = 4;
                nu.pntsv = 1;
                nu.orderu = 4;
                let xy = [[-1.5 * GRID, 0.0], [-GRID, GRID], [GRID, GRID], [1.5 * GRID, 0.0]];
                nu.bp = xy.iter().map(|p| bpoint([p[0], p[1], 0.0], 1.0)).collect();
                transform_all(mat, &mut nu.bp);
                if cutype == CU_NURBS {
                    nu.knotsu = Some(calc_knots(nu.pntsu, nu.orderu, nu.flagu));
                }
            }
        }
        CU_PRIM_PATH => {
            /* 5 point path */
            nu.pntsu = 5;
            nu.pntsv = 1;
            nu.orderu = 5;
            nu.flagu = CU_NURB_ENDPOINT;
            let x = [-2.0 * GRID, -GRID, 0.0, GRID, 2.0 * GRID];
            nu.bp = x.iter().map(|&x| bpoint([x, 0.0, 0.0], 1.0)).collect();
            transform_all(mat, &mut nu.bp);
            if cutype == CU_NURBS {
                nu.knotsu = Some(calc_knots(nu.pntsu, nu.orderu, nu.flagu));
            }
        }
        CU_PRIM_CIRCLE => {
            if cutype == CU_BEZIER {
                nu.pntsu = 4;
                nu.flagu = CU_NURB_CYCLIC;
                let keys = [[-GRID, 0.0, 0.0], [0.0, GRID, 0.0], [GRID, 0.0, 0.0], [0.0, -GRID, 0.0]];
                nu.bezt = keys.iter().map(|&k| bezt(HD_AUTO, tr3([[0.0; 3], k, [0.0; 3]]), 1.0, 1.0)).collect();
                handles_calc(&mut nu);
            } else if cutype == CU_NURBS {
                nu.pntsu = 8;
                nu.pntsv = 1;
                nu.orderu = 3;
                nu.flagu = CU_NURB_CYCLIC | CU_NURB_BEZIER | CU_NURB_ENDPOINT;
                nu.bp = NURBCIRCLE
                    .iter()
                    .enumerate()
                    .map(|(a, c)| bpoint([c[0] * GRID, c[1] * GRID, 0.0], if a & 1 != 0 { 0.5 * std::f32::consts::SQRT_2 } else { 1.0 }))
                    .collect();
                transform_all(mat, &mut nu.bp);
                nu.knotsu = Some(calc_knots(nu.pntsu, nu.orderu, nu.flagu));
            }
        }
        _ => return None,
    }
    Some(nu)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: [[f32; 4]; 4] = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

    fn close(a: [f32; 3], b: [f32; 3]) {
        assert!((0..3).all(|i| (a[i] - b[i]).abs() < 1e-4), "{a:?} vs {b:?}");
    }

    #[test]
    fn bezier_curve_has_two_aligned_selected_keys() {
        let nu = add_nurbs_primitive(&ID, CU_BEZIER | CU_PRIM_CURVE, 12).unwrap();
        assert_eq!((nu.pntsu, nu.resolu, nu.flagu), (2, 12, 0));
        let (a, b) = (nu.bezt[0], nu.bezt[1]);
        assert_eq!((a.bez.h1, a.bez.h2, a.f2, a.f3), (HD_ALIGN, HD_ALIGN, SELECT, SELECT));
        close(a.bez.vec[0], [-1.5, -0.5, 0.0]);
        close(a.bez.vec[1], [-1.0, 0.0, 0.0]);
        close(a.bez.vec[2], [-0.5, 0.5, 0.0]);
        close(b.bez.vec[0], [0.0, 0.0, 0.0]);
        close(b.bez.vec[1], [1.0, 0.0, 0.0]);
        close(b.bez.vec[2], [2.0, 0.0, 0.0]);
        assert_eq!((a.radius, a.weight, b.radius, b.weight), (1.0, 0.0, 1.0, 1.0), "the first key never gets a weight");
    }

    #[test]
    fn bezier_circle_has_four_auto_keys_with_round_handles() {
        let nu = add_nurbs_primitive(&ID, CU_BEZIER | CU_PRIM_CIRCLE, 12).unwrap();
        assert_eq!((nu.pntsu, nu.flagu), (4, CU_NURB_CYCLIC));
        let keys = [[-1.0, 0.0], [0.0, 1.0], [1.0, 0.0], [0.0, -1.0]];
        for (b, k) in nu.bezt.iter().zip(keys) {
            assert_eq!((b.bez.h1, b.bez.h2, b.radius, b.weight), (HD_AUTO, HD_AUTO, 1.0, 1.0));
            close(b.bez.vec[1], [k[0], k[1], 0.0]);
            /* The keys run clockwise; the handles are 0.5521 long along the tangent. */
            let t = [k[1], -k[0]];
            close(b.bez.vec[0], [k[0] - 0.552_125 * t[0], k[1] - 0.552_125 * t[1], 0.0]);
            close(b.bez.vec[2], [k[0] + 0.552_125 * t[0], k[1] + 0.552_125 * t[1], 0.0]);
        }
    }

    #[test]
    fn nurbs_curve_has_four_points_and_its_knots() {
        let nu = add_nurbs_primitive(&ID, CU_NURBS | CU_PRIM_CURVE, 12).unwrap();
        assert_eq!((nu.pntsu, nu.pntsv, nu.orderu, nu.flagu), (4, 1, 4, 0));
        let want = [[-1.5, 0.0], [-1.0, 1.0], [1.0, 1.0], [1.5, 0.0]];
        for (p, w) in nu.bp.iter().zip(want) {
            assert_eq!((p.vec, p.f1, p.radius, p.weight), ([w[0], w[1], 0.0, 1.0], SELECT, 1.0, 1.0));
        }
        assert_eq!(nu.knotsu.unwrap(), vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        /* A poly curve has the same points but no knots. */
        assert_eq!(add_nurbs_primitive(&ID, CU_POLY | CU_PRIM_CURVE, 12).unwrap().knotsu, None);
    }

    #[test]
    fn path_has_five_endpoint_points() {
        let nu = add_nurbs_primitive(&ID, CU_NURBS | CU_PRIM_PATH, 12).unwrap();
        assert_eq!((nu.pntsu, nu.orderu, nu.flagu), (5, 5, CU_NURB_ENDPOINT));
        let xs: Vec<f32> = nu.bp.iter().map(|p| p.vec[0]).collect();
        assert_eq!(xs, [-2.0, -1.0, 0.0, 1.0, 2.0]);
        assert_eq!(nu.knotsu.unwrap(), calc_knots(5, 5, CU_NURB_ENDPOINT));
    }

    #[test]
    fn nurbs_circle_has_eight_weighted_points() {
        let nu = add_nurbs_primitive(&ID, CU_NURBS | CU_PRIM_CIRCLE, 12).unwrap();
        assert_eq!((nu.pntsu, nu.orderu, nu.flagu), (8, 3, CU_NURB_CYCLIC | CU_NURB_BEZIER | CU_NURB_ENDPOINT));
        for (a, p) in nu.bp.iter().enumerate() {
            assert_eq!([p.vec[0], p.vec[1]], NURBCIRCLE[a]);
            let w = if a & 1 == 1 { std::f32::consts::FRAC_1_SQRT_2 } else { 1.0 };
            assert!((p.vec[3] - w).abs() < 1e-6, "weight of point {a}");
        }
        assert_eq!(nu.knotsu.unwrap().len(), crate::curve_knots::knot_count(8, 3, nu.flagu));
    }

    #[test]
    fn the_matrix_moves_every_point_and_handle() {
        let mut m = ID;
        m[3] = [10.0, 20.0, 30.0, 1.0];
        let nu = add_nurbs_primitive(&m, CU_BEZIER | CU_PRIM_CURVE, 12).unwrap();
        close(nu.bezt[0].bez.vec[1], [9.0, 20.0, 30.0]);
        close(nu.bezt[1].bez.vec[2], [12.0, 20.0, 30.0]);
        let nu = add_nurbs_primitive(&m, CU_NURBS | CU_PRIM_PATH, 12).unwrap();
        close([nu.bp[4].vec[0], nu.bp[4].vec[1], nu.bp[4].vec[2]], [12.0, 20.0, 30.0]);
    }

    #[test]
    fn unported_primitives_give_nothing() {
        assert!(add_nurbs_primitive(&ID, CU_NURBS | 0x400, 12).is_none());
    }
}
