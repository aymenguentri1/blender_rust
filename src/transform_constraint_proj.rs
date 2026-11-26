//! Port of `axisProjection()`, `planeProjection()`, `isPlaneProjectionViewAligned()` and
//! `viewAxisCorrectCenter()` (source/blender/editors/transform/transform_constraints.cc), with
//! `view_vector_calc()` (transform.cc) and `isect_ray_ray_v3()` / `isect_ray_plane_v3_factor()`
//! (blenlib/intern/math_geom.cc). `center` is `t->center_global`.

use crate::view3d::RegionView3D;

type V3 = [f32; 3];

fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn mul(a: V3, f: f32) -> V3 { [a[0] * f, a[1] * f, a[2] * f] }
fn len(a: V3) -> f32 { dot(a, a).sqrt() }
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn normalize(a: V3) -> V3 {
    let l = len(a);
    if dot(a, a) > 1.0e-35 { mul(a, 1.0 / l) } else { [0.0; 3] }
}
/// `project_v3_v3v3()`: `a` projected onto `b`.
fn project(a: V3, b: V3) -> V3 {
    let d = dot(b, b);
    if d == 0.0 { [0.0; 3] } else { mul(b, dot(a, b) / d) }
}
fn viewinv_row(rv3d: &RegionView3D, i: usize) -> V3 {
    [rv3d.viewinv[i][0], rv3d.viewinv[i][1], rv3d.viewinv[i][2]]
}

/// `view_vector_calc()`.
pub fn view_vector(rv3d: &RegionView3D, focus: V3) -> V3 {
    normalize(if rv3d.ortho { viewinv_row(rv3d, 2) } else { sub(viewinv_row(rv3d, 3), focus) })
}

/// `isect_ray_plane_v3_factor()`.
fn ray_plane_factor(o: V3, d: V3, plane_co: V3, plane_no: V3) -> Option<f32> {
    let dt = dot(plane_no, d);
    if dt == 0.0 { None } else { Some(-dot(plane_no, sub(o, plane_co)) / dt) }
}

/// `isect_ray_ray_v3()`: lambda along ray A of the closest point to ray B.
fn ray_ray_lambda(oa: V3, da: V3, ob: V3, db: V3) -> Option<f32> {
    let n = cross(db, da);
    let nlen = dot(n, n);
    if nlen < f32::MIN_POSITIVE {
        return None;
    }
    let c = sub(n, sub(ob, oa));
    Some(dot(cross(c, db), n) / nlen)
}

/// `viewAxisCorrectCenter()`: keeps the constraint center at least 1 unit in front of the view.
fn correct_center(rv3d: &RegionView3D, mut center: V3) -> V3 {
    let (v2, v3) = (viewinv_row(rv3d, 2), viewinv_row(rv3d, 3));
    let mut dir = sub(center, v3);
    if dot(dir, v2) < 0.0 {
        dir = mul(dir, -1.0);
    }
    let l = len(project(dir, v2));
    if l < 1.0 {
        center = sub(center, mul(normalize(v2), 1.0 - l));
    }
    center
}

/// `axisProjection()`: the motion `vec` projected onto the unit `axis` along the view.
pub fn axis_projection(rv3d: &RegionView3D, center: V3, axis: V3, vec: V3) -> V3 {
    if vec == [0.0; 3] {
        return [0.0; 3]; // Blender leaves `out` as the caller's fallback; callers pass zero.
    }
    let c = correct_center(rv3d, center);
    let v2 = viewinv_row(rv3d, 2);
    let cos = dot(normalize(axis), normalize(v2)).clamp(-1.0, 1.0);
    let mut angle = cos.acos().abs();
    if angle > std::f32::consts::FRAC_PI_2 {
        angle = std::f32::consts::PI - angle;
    }
    if angle < 5.0f32.to_radians() {
        /* View parallel to the axis: vertical motion drives it, quadratically. */
        let v1 = viewinv_row(rv3d, 1);
        let p = project(vec, v1);
        let mut factor = dot(v1, p) * 2.0;
        factor = if factor < 0.0 { factor * -factor } else { factor * factor };
        return mul(normalize(axis), -factor);
    }
    let plane = cross(view_vector(rv3d, c), axis);
    let v = add(sub(vec, project(vec, plane)), c);
    let norm = view_vector(rv3d, v);
    let factor = dot(axis, norm);
    if 1.0 - factor.abs() < 0.0002 {
        return mul(axis, if factor > 0.0 { 1.0e9 } else { -1.0e9 });
    }
    let out = ray_ray_lambda(c, axis, v, norm).map_or([0.0; 3], |m| mul(axis, m));
    out.map(|x| if x.is_finite() { x } else { 0.0 })
}

/// `isPlaneProjectionViewAligned()`.
pub fn plane_view_aligned(rv3d: &RegionView3D, center: V3, plane_no: V3) -> bool {
    dot(plane_no, view_vector(rv3d, center)).abs() < 0.001
}

/// `planeProjection()`: `out` stays `fallback` (Blender's `r_vec` from `pmtx`) when the view ray
/// is parallel to the plane.
pub fn plane_projection(rv3d: &RegionView3D, center: V3, plane_no: V3, vec: V3, fallback: V3) -> V3 {
    let pos = add(vec, center);
    let view_vec = view_vector(rv3d, pos);
    match ray_plane_factor(pos, view_vec, center, plane_no) {
        Some(f) => add(vec, mul(view_vec, f)),
        None => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ortho view looking down -Z (viewinv = identity, `viewinv[2]` = +Z toward the viewer).
    fn ortho() -> RegionView3D {
        RegionView3D { ortho: true, ..RegionView3D::default() }
    }
    fn near(a: V3, b: V3) { assert!((0..3).all(|i| (a[i] - b[i]).abs() < 1e-4), "{a:?} != {b:?}"); }

    #[test]
    fn view_vector_ortho_and_persp() {
        near(view_vector(&ortho(), [5.0, 5.0, 5.0]), [0.0, 0.0, 1.0]);
        let mut p = RegionView3D::default();
        p.viewinv[3] = [0.0, 0.0, 10.0, 1.0];
        near(view_vector(&p, [0.0, 0.0, 0.0]), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn axis_projection_in_an_ortho_view_keeps_the_axis_component() {
        let r = axis_projection(&ortho(), [0.0; 3], [1.0, 0.0, 0.0], [2.0, 3.0, 0.0]);
        near(r, [2.0, 0.0, 0.0]);
        let r = axis_projection(&ortho(), [0.0; 3], [0.0, 1.0, 0.0], [2.0, -3.0, 0.0]);
        near(r, [0.0, -3.0, 0.0]);
    }

    #[test]
    fn axis_projection_of_zero_is_zero() {
        near(axis_projection(&ortho(), [0.0; 3], [1.0, 0.0, 0.0], [0.0; 3]), [0.0; 3]);
    }

    #[test]
    fn axis_parallel_to_the_view_uses_vertical_motion_quadratically() {
        let r = axis_projection(&ortho(), [0.0; 3], [0.0, 0.0, 1.0], [0.0, 0.5, 0.0]);
        near(r, [0.0, 0.0, -1.0]); // factor = (0.5*2)^2 = 1, moves along -axis
        let r = axis_projection(&ortho(), [0.0; 3], [0.0, 0.0, 1.0], [0.0, -0.5, 0.0]);
        near(r, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn plane_projection_follows_the_view_ray() {
        let v = ortho();
        assert!(!plane_view_aligned(&v, [0.0; 3], [0.0, 0.0, 1.0]));
        assert!(plane_view_aligned(&v, [0.0; 3], [1.0, 0.0, 0.0]));
        // XY plane seen from +Z: the motion stays in the plane.
        near(plane_projection(&v, [0.0; 3], [0.0, 0.0, 1.0], [1.0, 2.0, 0.0], [9.0; 3]), [1.0, 2.0, 0.0]);
        // A motion with a Z part is slid along the view ray back onto the plane.
        near(plane_projection(&v, [0.0; 3], [0.0, 0.0, 1.0], [1.0, 2.0, 3.0], [9.0; 3]), [1.0, 2.0, 0.0]);
        // Parallel ray: fallback.
        near(plane_projection(&v, [0.0; 3], [1.0, 0.0, 0.0], [1.0, 2.0, 3.0], [9.0; 3]), [9.0; 3]);
    }
}
