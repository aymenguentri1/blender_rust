//! Ray to AABB distance (source/blender/blenlib/intern/math_geom.cc: `isect_ray_line_v3`, `dist_squared_ray_to_seg_v3`,
//! `aabb_get_near_far_from_plane`, `dist_squared_ray_to_aabb_v3(_precalc)`).

pub type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `isect_ray_line_v3`: `lambda` along `v0 -> v1` of the point nearest to the ray; `None` when parallel.
pub fn isect_ray_line(ray_origin: V3, ray_direction: V3, v0: V3, v1: V3) -> Option<f32> {
    let a = sub(v1, v0);
    let t = sub(v0, ray_origin);
    let n = cross(a, ray_direction);
    let nlen = dot(n, n);
    if nlen == 0.0 {
        return None;
    }
    let c = sub(n, t);
    let cray = cross(c, ray_direction);
    Some(dot(cray, n) / nlen)
}

/// `dist_squared_ray_to_seg_v3`: `(distance squared, nearest point on the segment, depth along the ray)`.
pub fn dist_squared_ray_to_seg(ray_origin: V3, ray_direction: V3, v0: V3, v1: V3) -> (f32, V3, f32) {
    let point = match isect_ray_line(ray_origin, ray_direction, v0, v1) {
        Some(l) if l <= 0.0 => v0,
        Some(l) if l >= 1.0 => v1,
        Some(l) => [v0[0] + (v1[0] - v0[0]) * l, v0[1] + (v1[1] - v0[1]) * l, v0[2] + (v1[2] - v0[2]) * l],
        None => v0,
    };
    let dvec = sub(point, ray_origin);
    let depth = dot(dvec, ray_direction);
    (dot(dvec, dvec) - depth * depth, point, depth)
}

/// `aabb_get_near_far_from_plane`: `(near, far)` corners of the box for a plane normal.
pub fn aabb_near_far_from_plane(plane_no: V3, bbmin: V3, bbmax: V3) -> (V3, V3) {
    let mut near = [0.0; 3];
    let mut far = [0.0; 3];
    for k in 0..3 {
        if plane_no[k] < 0.0 {
            near[k] = bbmax[k];
            far[k] = bbmin[k];
        } else {
            near[k] = bbmin[k];
            far[k] = bbmax[k];
        }
    }
    (near, far)
}

/// `DistRayAABB_Precalc`.
#[derive(Clone, Copy, Debug)]
pub struct DistRayAabbPrecalc {
    pub ray_origin: V3,
    pub ray_direction: V3,
    pub ray_inv_dir: V3,
}

/// `dist_squared_ray_to_aabb_v3_precalc`.
pub fn dist_ray_aabb_precalc(ray_origin: V3, ray_direction: V3) -> DistRayAabbPrecalc {
    let mut inv = [0.0; 3];
    for k in 0..3 {
        inv[k] = if ray_direction[k] != 0.0 { 1.0 / ray_direction[k] } else { f32::MAX };
    }
    DistRayAabbPrecalc { ray_origin, ray_direction, ray_inv_dir: inv }
}

/// `dist_squared_ray_to_aabb_v3`: `(distance squared, nearest point, depth)`; 0 when the ray crosses the box.
pub fn dist_squared_ray_to_aabb(d: &DistRayAabbPrecalc, bb_min: V3, bb_max: V3) -> (f32, V3, f32) {
    let (lmin, lmax) = aabb_near_far_from_plane(d.ray_direction, bb_min, bb_max);
    let mut tmin = [0.0; 3];
    let mut tmax = [0.0; 3];
    for k in 0..3 {
        tmin[k] = (lmin[k] - d.ray_origin[k]) * d.ray_inv_dir[k];
        tmax[k] = (lmax[k] - d.ray_origin[k]) * d.ray_inv_dir[k];
    }
    let mut va = [0.0f32; 3];
    let mut vb = [0.0f32; 3];
    let rtmax;
    let rtmin;
    let mut main_axis: i32;

    if tmax[0] <= tmax[1] && tmax[0] <= tmax[2] {
        rtmax = tmax[0];
        va[0] = lmax[0];
        vb[0] = lmax[0];
        main_axis = 3;
    } else if tmax[1] <= tmax[0] && tmax[1] <= tmax[2] {
        rtmax = tmax[1];
        va[1] = lmax[1];
        vb[1] = lmax[1];
        main_axis = 2;
    } else {
        rtmax = tmax[2];
        va[2] = lmax[2];
        vb[2] = lmax[2];
        main_axis = 1;
    }

    if tmin[0] >= tmin[1] && tmin[0] >= tmin[2] {
        rtmin = tmin[0];
        va[0] = lmin[0];
        vb[0] = lmin[0];
        main_axis -= 3;
    } else if tmin[1] >= tmin[0] && tmin[1] >= tmin[2] {
        rtmin = tmin[1];
        va[1] = lmin[1];
        vb[1] = lmin[1];
        main_axis -= 1;
    } else {
        rtmin = tmin[2];
        va[2] = lmin[2];
        vb[2] = lmin[2];
        main_axis -= 2;
    }
    if main_axis < 0 {
        main_axis += 3;
    }
    let m = main_axis as usize;

    if rtmin <= rtmax {
        let depth = dot(sub(lmax, d.ray_origin), d.ray_direction);
        return (0.0, lmax, depth);
    }

    if d.ray_direction[m] >= 0.0 {
        va[m] = lmin[m];
        vb[m] = lmax[m];
    } else {
        va[m] = lmax[m];
        vb[m] = lmin[m];
    }
    dist_squared_ray_to_seg(d.ray_origin, d.ray_direction, va, vb)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: V3 = [0.0; 3];
    const MAX: V3 = [1.0; 3];

    fn dist(o: V3, dir: V3) -> f32 {
        dist_squared_ray_to_aabb(&dist_ray_aabb_precalc(o, dir), MIN, MAX).0
    }

    #[test]
    fn ray_through_box_is_zero() {
        assert_eq!(dist([-5.0, 0.5, 0.5], [1.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn ray_beside_box_measures_gap() {
        assert!((dist([-5.0, 3.0, 0.5], [1.0, 0.0, 0.0]) - 4.0).abs() < 1e-5);
        assert!((dist([-5.0, 0.5, 3.0], [1.0, 0.0, 0.0]) - 4.0).abs() < 1e-5);
        assert!((dist([-5.0, 3.0, 3.0], [1.0, 0.0, 0.0]) - 8.0).abs() < 1e-5);
    }

    #[test]
    fn seg_distance_parallel_and_clamped() {
        let (d, p, _) = dist_squared_ray_to_seg([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [5.0, 2.0, 0.0]);
        assert!((d - 4.0).abs() < 1e-6 && p == [0.0, 2.0, 0.0]);
        let (d, p, _) = dist_squared_ray_to_seg([0.0; 3], [1.0, 0.0, 0.0], [3.0, 1.0, 1.0], [3.0, 1.0, 5.0]);
        assert!((d - 2.0).abs() < 1e-6 && p == [3.0, 1.0, 1.0]);
    }

    #[test]
    fn near_far_corners_follow_plane_sign() {
        let (n, f) = aabb_near_far_from_plane([-1.0, 1.0, 0.0], [0.0; 3], [1.0, 2.0, 3.0]);
        assert_eq!((n, f), ([1.0, 0.0, 0.0], [0.0, 2.0, 3.0]));
    }
}
