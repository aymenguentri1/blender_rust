//! Port of source/blender/editors/space_view3d/view3d_placement.cc (plane + snap math)
//! and math_geom.cc (plane_from_point_normal_v3, isect_ray_plane_v3_factor).

pub type V3 = [f32; 3];
pub type Plane = [f32; 4];

/// view3d_placement.cc: `eps_view_align`.
pub const EPS_VIEW_ALIGN: f32 = 1e-2;

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// math_geom.cc `plane_from_point_normal_v3`.
pub fn plane_from_point_normal(co: V3, no: V3) -> Plane {
    [no[0], no[1], no[2], -dot(no, co)]
}

/// math_geom.cc `isect_ray_plane_v3_factor`: lambda along the ray, None if parallel (dot == 0).
pub fn ray_plane_factor(origin: V3, dir: V3, plane_co: V3, plane_no: V3) -> Option<f32> {
    let d = dot(plane_no, dir);
    if d == 0.0 {
        return None;
    }
    let h = [origin[0] - plane_co[0], origin[1] - plane_co[1], origin[2] - plane_co[2]];
    Some(-dot(plane_no, h) / d)
}

/// Ray hit point on the plane through `plane_co` with normal `plane_no`.
pub fn ray_plane_point(origin: V3, dir: V3, plane_co: V3, plane_no: V3) -> Option<V3> {
    let l = ray_plane_factor(origin, dir, plane_co, plane_no)?;
    Some([origin[0] + dir[0] * l, origin[1] + dir[1] * l, origin[2] + dir[2] * l])
}

/// view3d_interactive_add_begin: (STEP_BASE, STEP_DEPTH) degenerate view-align flags.
/// `view_z` is rv3d->viewinv[2], `orient_axis` is matrix_orient[plane_axis].
pub fn degenerate_view_align(view_z: V3, orient_axis: V3) -> (bool, bool) {
    let dot = dot(view_z, orient_axis).abs();
    (dot < EPS_VIEW_ALIGN, (dot - 1.0).abs() < EPS_VIEW_ALIGN)
}

/// `idp_snap_calc_incremental`: round `co` to `grid_size`, relative to `co_relative`
/// unless grid snapping is active (`snap_to_grid`). Returns None when grid_size == 0.
pub fn snap_incremental(co: V3, co_relative: Option<V3>, snap_to_grid: bool, grid_size: f32) -> Option<V3> {
    if grid_size == 0.0 {
        return None;
    }
    let rel = if snap_to_grid { None } else { co_relative };
    let r = rel.unwrap_or([0.0; 3]);
    let mut out = [0.0; 3];
    for i in 0..3 {
        out[i] = ((co[i] - r[i]) * (1.0 / grid_size)).round() * grid_size + r[i];
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_equation() {
        let p = plane_from_point_normal([0., 0., 2.], [0., 0., 1.]);
        assert_eq!(p, [0., 0., 1., -2.]);
    }

    #[test]
    fn ray_hits_plane_at_expected_point() {
        let hit = ray_plane_point([1., 2., 5.], [0., 0., -1.], [0., 0., 1.], [0., 0., 1.]).unwrap();
        assert_eq!(hit, [1., 2., 1.]);
    }

    #[test]
    fn parallel_ray_gives_none() {
        assert!(ray_plane_point([0., 0., 5.], [1., 0., 0.], [0., 0., 1.], [0., 0., 1.]).is_none());
    }

    #[test]
    fn degenerate_flags() {
        assert_eq!(degenerate_view_align([0., 0., 1.], [0., 0., 1.]), (false, true));
        assert_eq!(degenerate_view_align([1., 0., 0.], [0., 0., 1.]), (true, false));
        assert_eq!(degenerate_view_align([0., 0.6, 0.8], [0., 0., 1.]), (false, false));
    }

    #[test]
    fn increment_snap_rounds_to_step() {
        let s = snap_incremental([1.2, 2.7, -0.4], None, false, 0.5).unwrap();
        assert_eq!(s, [1.0, 2.5, -0.5]);
        let r = snap_incremental([1.2, 0.0, 0.0], Some([0.1, 0.0, 0.0]), false, 1.0).unwrap();
        assert!((r[0] - 1.1).abs() < 1e-6);
        let g = snap_incremental([1.2, 0.0, 0.0], Some([0.1, 0.0, 0.0]), true, 1.0).unwrap();
        assert_eq!(g[0], 1.0);
        assert!(snap_incremental([1., 1., 1.], None, false, 0.0).is_none());
    }
}
